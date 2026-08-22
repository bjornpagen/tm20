use paper_attention_router::{
    Digest, DigestItem, Interrupt, MAX_TAPE_DOTS, render_digest, render_interrupt,
};
use paper_attention_router::paper::Section;

#[test]
fn interrupt_specimen_is_locked() {
    let interrupt = Interrupt::parse(
        "Now",
        "iMessage",
        "Ada",
        "now",
        "Can you call when the train arrives?",
        "I-0001",
    )
    .expect("valid interrupt");
    let rendered = render_interrupt(&interrupt).expect("bounded interrupt");
    assert_eq!(
        rendered.markdown.trim_end(),
        include_str!("../specimens/interrupt.md").trim_end()
    );
    assert!(rendered.estimated_height_dots <= MAX_TAPE_DOTS);
}

#[test]
fn digest_specimen_is_locked() {
    let items = vec![
        DigestItem::parse(
            Section::People,
            "iMessage",
            "Ada",
            "2m",
            "Train reaches Central at 12:48.",
            1,
        )
        .expect("people"),
        DigestItem::parse(
            Section::Work,
            "Slack",
            "buildbot",
            "4m",
            "Main is green after the parser change.",
            3,
        )
        .expect("work"),
        DigestItem::parse(
            Section::Mail,
            "Gmail",
            "Bank",
            "10m",
            "Statement available; no action due.",
            1,
        )
        .expect("mail"),
        DigestItem::parse(
            Section::Network,
            "HN",
            "item 412",
            "32m",
            "A careful thread on typed plugins.",
            1,
        )
        .expect("network"),
    ];
    let digest = Digest::parse("Now · 12:30", "D-0042", items, 7).expect("digest");
    let rendered = render_digest(&digest);
    assert_eq!(
        rendered.markdown.trim_end(),
        include_str!("../specimens/digest.md").trim_end()
    );
    assert_eq!(rendered.omitted, 3);
    assert!(rendered.estimated_height_dots <= MAX_TAPE_DOTS);
}
