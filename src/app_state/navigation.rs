use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle, Styled, Triangle};
use embedded_graphics::text::Text;

pub type StyledTriangle = Styled<Triangle, PrimitiveStyle<BinaryColor>>;
pub type StyledRectangle = Styled<Rectangle, PrimitiveStyle<BinaryColor>>;
pub type StyledText<'s, 't> = Text<'t, MonoTextStyle<'s, BinaryColor>>;

#[allow(unused)]
pub enum NavigationIcons {
    Up,
    Down,
    Left,
    Right,
    Center,
}

pub struct NavigationDrawables {
    up: StyledTriangle,
    down: StyledTriangle,
    left: StyledTriangle,
    right: StyledTriangle,
    center: StyledRectangle,
}

impl NavigationDrawables {
    pub fn new(style: &PrimitiveStyle<BinaryColor>) -> Self {
        Self {
            up: Triangle::new(Point::new(-3, 0), Point::new(3, 3), Point::new(3, -3))
                .into_styled(style.clone()),
            down: Triangle::new(Point::new(-3, 0), Point::new(3, 3), Point::new(3, -3))
                .into_styled(style.clone()),
            left: Triangle::new(Point::new(-3, 0), Point::new(3, 3), Point::new(3, -3))
                .into_styled(style.clone()),
            right: Triangle::new(Point::new(3, 0), Point::new(-3, 3), Point::new(-3, -3))
                .into_styled(style.clone()),
            center: Rectangle::new(Point::new(-3, -3), Size::new(6, 6)).into_styled(style.clone()),
        }
    }

    pub fn draw_icon<D: DrawTarget<Color = BinaryColor>>(
        &self,
        target: &mut D,
        icon: NavigationIcons,
        position: Point,
    ) -> Result<(), D::Error> {
        use NavigationIcons::*;
        match icon {
            Up => self.up.translate(position).draw(target),
            Down => self.down.translate(position).draw(target),
            Left => self.left.translate(position).draw(target),
            Right => self.right.translate(position).draw(target),
            Center => self.center.translate(position).draw(target),
        }
    }

    pub fn draw_icon_and_text<D: DrawTarget<Color = BinaryColor>>(
        &self,
        target: &mut D,
        icon: NavigationIcons,
        position: Point,
        mut text: StyledText<'_, '_>,
    ) -> Result<(), D::Error> {
        use NavigationIcons::*;
        match icon {
            Up => self.up.translate(position).draw(target),
            Down => self.down.translate(position).draw(target),
            Left => self.left.translate(position).draw(target),
            Right => self.right.translate(position).draw(target),
            Center => self.center.translate(position).draw(target),
        }?;

        // Assume all icons 6 x 6 so draw text in position + 6 pixels
        text.position = Point {
            x: position.x + 6,
            y: position.y + 3, // Center text
        };
        text.draw(target)?;

        Ok(())
    }
}
