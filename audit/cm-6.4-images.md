# Images

- Spec: https://spec.commonmark.org/0.31.2/#images
- Status: **gap**
- Walk: a paragraph whose only child is `Image` loads bytes and becomes `Frame::Figure` (`from_image`, fill measure). Alt text is discarded. Any other image in a paragraph, or `Image` reached from `inline` (including `[![alt](src)](href)`), is `Error::MixedImage`. Failed load is `Error::Image`.
- Proof: `spec.rs::image_paragraph_is_a_figure`, `mixed_text_and_image_is_an_error`; fixture `10-figure.md`
- Later do: either never for inline images (keep MixedImage) or a `Span` box like math; linked figures miss the sole-child path today.
