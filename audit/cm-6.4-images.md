# Images

- Spec: https://spec.commonmark.org/0.31.2/#images
- Status: **keep**
- Corpus: `cm-6.4-a-images`, `set-fig-a-native`, `set-fig-b-measure-edge`, `set-fig-c-extreme-aspect`, `set-fig-d-dither`, `set-fig-e-modes`, `set-fig-f-jpeg`, `rej-image-a-mixed`, `rej-image-b-remote`, `rej-image-c-missing`, `rej-image-d-garbage`
- Walk: a paragraph whose only child is `Image` loads bytes and becomes `Frame::Figure` (`from_image`, fill measure). Alt text is discarded. Any other image in a paragraph, or `Image` reached from `inline` (including `[![alt](src)](href)`), is `Error::MixedImage`. Failed load is `Error::Image`.
- Proof: golden `cm-6.4-a-images`; figure stress `set-fig-a`…`f`; rejects `rej-image-a`…`d`; `spec.rs` image facts; fixture `10-figure.md`.
- Later do: none. A mixed paragraph is `Error::MixedImage`; that is the dialect.
