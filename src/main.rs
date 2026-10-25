use std::{collections::HashMap, fs::File};

use ab_glyph::{FontRef, PxScale};
use clap::Parser;
use image::{Pixel, Rgb, RgbImage};
use imageproc::{
    definitions::Clamp,
    drawing::{
        Canvas, draw_filled_rect_mut, draw_hollow_rect_mut, draw_line_segment_mut, draw_text_mut,
        text_size,
    },
    rect::Rect,
};
use serde::{Deserialize, Serialize};

#[derive(Parser, Debug)]
struct Args {
    #[command(flatten)]
    screen: Size,

    /// path to the ui file
    ui: String,
}

#[derive(clap::Args, Deserialize, Clone, Debug)]
struct Size {
    /// width of the image
    #[arg(short, long, default_value_t = 800)]
    width: u32,
    /// heigth of the image
    #[arg(short('x'), long, default_value_t = 480)]
    height: u32,
}

#[derive(Deserialize, Clone, Copy, Debug, Serialize)]
enum Value {
    Relative(f64),
    Absolute(u32),
    Min,
    Max,
    Center,
}

impl Value {
    fn get(self, size: u32, own_size: u32) -> u32 {
        match self {
            Value::Relative(v) => (v * size as f64) as u32,
            Value::Absolute(v) => v,
            Value::Min => 0,
            Value::Max => size.saturating_sub(own_size),
            Value::Center => size.saturating_sub(own_size) >> 1,
        }
    }
}

#[derive(Deserialize, Clone, Debug, Serialize)]
struct Position {
    x: Value,
    y: Value,
}

#[derive(Deserialize, Clone, Debug)]
struct Text {
    pub text: String,
    #[serde(flatten)]
    pub pos: Position,
    pub size: f32,
}

#[derive(Deserialize, Clone, Debug)]
struct Line {
    pub p1: Position,
    pub p2: Position,
}
impl Line {
    fn get(&self, area: &Area) -> ((f32, f32), (f32, f32)) {
        let size = Size {
            width: 1,
            height: 1,
        };

        let p1 = self.p1.get(&area, &size);
        let p2 = self.p2.get(&area, &size);
        ((p1.0 as f32, p1.1 as f32), (p2.0 as f32, p2.1 as f32))
    }
}

#[derive(Deserialize, Clone, Debug)]
struct Rectangle {
    #[serde(flatten)]
    pub pos: Position,
    pub width: Value,
    pub height: Value,
    #[serde(default)]
    pub fill: bool,
}
impl Rectangle {
    fn get(&self, area: &Area) -> Rect {
        let width = self.width.get(area.width, 0);
        let height = self.height.get(area.height, 0);
        let (x, y) = self.pos.get(area, &Size { width, height });

        Rect::at(x as i32, y as i32).of_size(width, height)
    }
}

#[derive(Deserialize, Clone, Debug)]
struct Section {
    #[serde(flatten)]
    pub pos: Position,
    height: Value,
    width: Value,
    items: Vec<Item>,
}
impl Section {
    fn to_area(&self, area: &Area) -> Area {
        let width = self.width.get(area.width, 0);
        let height = self.height.get(area.height, 0);
        let (x, y) = self.pos.get(area, &Size { width, height });
        Area {
            x,
            y,
            height,
            width,
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
enum Item {
    Text(Text),
    Rectangle(Rectangle),
}

#[derive(Clone, Debug)]
struct Area {
    x: u32,
    y: u32,
    height: u32,
    width: u32,
}

#[derive(Deserialize, Clone, Debug)]
struct Ui {
    sections: Vec<Section>,
    #[serde(default)]
    rectangles: Vec<Rectangle>,
}

impl Position {
    pub fn get(&self, area: &Area, own_size: &Size) -> (u32, u32) {
        (
            self.x.get(area.width, own_size.width) + area.x,
            self.y.get(area.height, own_size.height) + area.y,
        )
    }
}

fn draw_text<C>(
    item: &Text,
    area: &Area,
    brush: C::Pixel,
    data: &HashMap<String, String>,
    image: &mut C,
    font: &FontRef<'_>,
) where
    C: Canvas,
    <C::Pixel as Pixel>::Subpixel: Into<f32> + Clamp<f32>,
{
    let scale = PxScale {
        x: item.size,
        y: item.size,
    };

    let mut text = item.text.clone();
    for (key, value) in data.iter() {
        let key = format!("{{{key}}}");
        text = text.replace(&key, value);
    }
    let (w, h) = text_size(scale, &font, &text);
    let (x, y) = item.pos.get(
        &area,
        &Size {
            width: w,
            height: h,
        },
    );
    println!("text to draw at ({x}, {y})");
    draw_text_mut(image, brush, x as i32, y as i32, scale, &font, &text);
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let mut image = RgbImage::new(args.screen.width, args.screen.height);
    image.fill(255u8);

    let font = FontRef::try_from_slice(include_bytes!(
        "../3270NerdFont-Regular.ttf" // "/usr/share/fonts/open-sans/OpenSans-Regular.ttf"
    ))?;

    let ui: Ui = serde_json::from_reader(File::open(args.ui)?)?;

    let data: HashMap<String, String> = serde_json::from_reader(File::open("data.json")?)?;
    let area = Area {
        x: 0,
        y: 0,
        width: args.screen.width,
        height: args.screen.height,
    };

    let brush = Rgb([0u8, 0u8, 0u8]);

    for section in ui.sections {
        let area = section.to_area(&area);
        println!("area to draw {area:?}");
        for item in section.items.iter() {
            match item {
                Item::Text(text) => draw_text(text, &area, brush, &data, &mut image, &font),
                Item::Rectangle(rectangle) => {
                    let rect = rectangle.get(&area);
                    if rectangle.fill {
                        draw_filled_rect_mut(&mut image, rect, brush);
                    } else {
                        draw_hollow_rect_mut(&mut image, rect, brush);
                    }
                }
            }
        }
    }

    for rectangle in ui.rectangles {
        let rect = rectangle.get(&area);
        if rectangle.fill {
            draw_filled_rect_mut(&mut image, rect, brush);
        } else {
            draw_hollow_rect_mut(&mut image, rect, brush);
        }
    }

    // for t in ui.lines {
    //     let (start, end) = t.get(&area);
    //     println!("start {start:?}, end:{end:?}");
    //     draw_line_segment_mut(&mut image, start, end, brush);
    // }

    image.save("output.png")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let x = Value::Min;
        let y = Value::Relative(0.3);
        let p = Position { x, y };

        let out = serde_json::to_string(&p);
        println!("{:?}", out);
    }
}
