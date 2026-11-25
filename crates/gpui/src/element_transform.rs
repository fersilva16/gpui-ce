use crate::{Bounds, ContentMask, Pixels, Point, ScaledPixels, TransformationMatrix, point};

/// A uniform scale followed by a translation, in logical pixels: what
/// [`Window::with_element_transform`](crate::Window::with_element_transform) applies to an
/// element and everything it paints, without changing its layout.
///
/// A point `p` maps to `p * scale + translation`. The scale stays positive, so a transformed
/// rectangle is still an axis-aligned rectangle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ElementTransform {
    /// The scale about the window's origin.
    pub scale: f32,
    /// Where the window's origin lands after the scale.
    pub translation: Point<Pixels>,
}

impl Default for ElementTransform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl ElementTransform {
    /// The transform that leaves everything where it is.
    pub const IDENTITY: Self = Self {
        scale: 1.,
        translation: Point {
            x: Pixels(0.),
            y: Pixels(0.),
        },
    };

    /// A scale by `scale` about `origin`, which stays where it is.
    pub fn scale_about(scale: f32, origin: Point<Pixels>) -> Self {
        Self {
            scale,
            translation: origin - origin * scale,
        }
    }

    /// A move by `offset`.
    pub fn translate(offset: Point<Pixels>) -> Self {
        Self {
            scale: 1.,
            translation: offset,
        }
    }

    /// This transform, then `after`.
    pub fn then(self, after: Self) -> Self {
        Self {
            scale: self.scale * after.scale,
            translation: after.apply(self.translation),
        }
    }

    /// The transform that undoes this one.
    pub fn inverse(self) -> Self {
        let scale = 1. / self.scale;
        Self {
            scale,
            translation: self.translation * -scale,
        }
    }

    /// Whether it leaves everything where it is.
    pub fn is_identity(&self) -> bool {
        *self == Self::IDENTITY
    }

    /// Where `point` lands.
    pub fn apply(&self, point: Point<Pixels>) -> Point<Pixels> {
        point * self.scale + self.translation
    }

    /// Where `bounds` land.
    pub fn apply_bounds(&self, bounds: Bounds<Pixels>) -> Bounds<Pixels> {
        Bounds {
            origin: self.apply(bounds.origin),
            size: bounds.size.map(|length| length * self.scale),
        }
    }

    /// Where `mask` lands: its bounds move and its edge fades scale.
    pub fn apply_mask(&self, mask: ContentMask<Pixels>) -> ContentMask<Pixels> {
        ContentMask {
            bounds: self.apply_bounds(mask.bounds),
            fade_out: mask.fade_out.map(|fade| *fade * self.scale),
        }
    }

    /// `matrix`, a transformation in device pixels of what is drawn untransformed, made to act
    /// on what this transform drew instead: `self * matrix * self⁻¹`, where `scale_factor` turns
    /// this transform's translation into device pixels.
    pub(crate) fn conjugate(
        &self,
        matrix: TransformationMatrix,
        scale_factor: f32,
    ) -> TransformationMatrix {
        let t = self.translation.scale(scale_factor);
        let [[a, b], [c, d]] = matrix.rotation_scale;
        let moved: Point<ScaledPixels> = point(
            ScaledPixels(a * t.x.0 + b * t.y.0),
            ScaledPixels(c * t.x.0 + d * t.y.0),
        );
        let [x, y] = matrix.translation;
        TransformationMatrix {
            rotation_scale: matrix.rotation_scale,
            translation: [
                x * self.scale + t.x.0 - moved.x.0,
                y * self.scale + t.y.0 - moved.y.0,
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Edges, px, radians, size};

    #[test]
    fn a_scale_about_a_point_keeps_that_point_and_moves_the_rest() {
        let t = ElementTransform::scale_about(0.5, point(px(10.), px(20.)));
        assert_eq!(t.apply(point(px(10.), px(20.))), point(px(10.), px(20.)));
        assert_eq!(t.apply(point(px(30.), px(40.))), point(px(20.), px(30.)));
        assert_eq!(
            t.apply_bounds(Bounds::new(point(px(10.), px(20.)), size(px(40.), px(20.)))),
            Bounds::new(point(px(10.), px(20.)), size(px(20.), px(10.)))
        );
    }

    #[test]
    fn composing_applies_the_inner_transform_first() {
        let scale = ElementTransform::scale_about(2., point(px(0.), px(0.)));
        let shift = ElementTransform::translate(point(px(5.), px(0.)));
        let p = point(px(1.), px(1.));
        assert_eq!(scale.then(shift).apply(p), shift.apply(scale.apply(p)));
        assert_eq!(shift.then(scale).apply(p), scale.apply(shift.apply(p)));
        assert_eq!(scale.then(shift).apply(p), point(px(7.), px(2.)));
    }

    #[test]
    fn the_inverse_undoes_the_transform() {
        let t = ElementTransform::scale_about(0.94, point(px(100.), px(50.)))
            .then(ElementTransform::translate(point(px(0.), px(8.))));
        let p = point(px(37.), px(-12.));
        let back = t.inverse().apply(t.apply(p));
        assert!((back.x - p.x).abs() < px(1e-4) && (back.y - p.y).abs() < px(1e-4));
        let undone = t.then(t.inverse());
        assert!((undone.scale - 1.).abs() < 1e-6, "{undone:?}");
        assert!(undone.translation.x.abs() < px(1e-4) && undone.translation.y.abs() < px(1e-4));
    }

    #[test]
    fn a_mask_moves_with_its_fades_scaled() {
        let t = ElementTransform::scale_about(0.5, point(px(0.), px(0.)));
        let mask = ContentMask {
            bounds: Bounds::new(point(px(10.), px(10.)), size(px(100.), px(100.))),
            fade_out: Edges {
                top: px(8.),
                ..Edges::default()
            },
        };
        let moved = t.apply_mask(mask);
        assert_eq!(
            moved.bounds,
            Bounds::new(point(px(5.), px(5.)), size(px(50.), px(50.)))
        );
        assert_eq!(moved.fade_out.top, px(4.));
    }

    #[test]
    fn a_rotation_conjugated_turns_about_where_its_center_was_drawn() {
        let scale_factor = 2.;
        let center = point(px(10.), px(10.));
        let rotation = TransformationMatrix::unit()
            .translate(center.scale(scale_factor))
            .rotate(radians(std::f32::consts::FRAC_PI_2))
            .translate(center.scale(-scale_factor));
        let t = ElementTransform::scale_about(0.5, point(px(0.), px(0.)))
            .then(ElementTransform::translate(point(px(4.), px(0.))));
        let conjugated = t.conjugate(rotation, scale_factor);
        let drawn_center = t.apply(center) * scale_factor;
        let turned = conjugated.apply(drawn_center);
        assert!((turned.x - drawn_center.x).abs() < px(1e-4), "{turned:?}");
        assert!((turned.y - drawn_center.y).abs() < px(1e-4), "{turned:?}");
    }
}
