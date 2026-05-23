---
name: alt-text
description: Writes alt text for images the user is about to post on social media (Twitter/X, Bluesky, Instagram, LinkedIn, Mastodon, Threads, Facebook, etc.), following accessibility best practices on length, detail, and tone. Trigger this whenever the user shares an image and asks for "alt text", "alt", "image description", "a11y description", "screen reader description", or mentions posting/sharing an image with phrasing like "for my Bluesky post" or "I'm about to tweet this". Trigger even when the request is minimal — e.g., the user just drops an image with "alt?" or "for posting" — the intent is implicit. Do NOT trigger when the user wants to understand an image for themselves ("what's in this picture", "describe this to me") or wants a visible caption rather than alt text.
---

# Alt text for social media

Write alt text that a screen reader user would actually want to hear. Most AI-generated alt text fails by being exhaustive but pointless ("A young woman with long brown hair wearing a denim jacket stands in front of a beige wall holding a coffee cup..."). That's not alt text — that's a deposition. Good alt text conveys what matters about the image, fast, in the voice of someone who actually looked at it and is telling a friend what's there.

## Mental model

The question to keep asking: **if this image disappeared, what would the post lose?** That's what the alt text needs to carry. Everything else is noise.

A screen reader user is consuming a feed. They want the *point* of the image, not a forensic inventory. If the image is the punchline, the alt text needs to land the punchline. If the image is a chart, the alt text needs to convey the takeaway. If the image is a selfie at a concert, the alt text needs to convey the vibe and what's identifiable, not every garment.

## Length and structure

Aim for roughly **1,000 characters** by default. Go shorter when the image is simple and a sentence or two genuinely covers it; go longer only when the image carries even more meaning than that (dense charts, multi-panel layouts, long screenshots of text). Don't pad to hit a target, and don't truncate something that genuinely needs more.

Platform character limits, for awareness: Bluesky 2,000, Mastodon 1,500, X/Twitter 1,000, LinkedIn 120 (yes, really short), Instagram ~125 visible (longer technically allowed). The 1,000-character default fits X/Twitter and stays under the larger platforms; trim hard for LinkedIn/Instagram when the user names them.

Default structure, loosely: **subject → action/setting → relevant detail → any text in image**. But this is a guide, not a template. Real alt text reads like a sentence or two of natural prose, not a labeled form.

## What to include, what to skip

Include:
- The subject and what they're doing
- Setting/context if it matters to the post
- **Any text visible in the image** — transcribe it. Signs, captions on memes, tweet screenshots, chart labels, slide titles, product names. This is the most common failure point.
- Mood or atmosphere if the image is artistic or that's the point
- Identifying details about people when relevant (see below)

Skip:
- "Image of", "Photo of", "Picture showing" — the screen reader already says it's an image
- Hedging: "appears to be", "looks like", "what seems to be" — write directly. If you're not sure, pick the most likely reading and commit, or ask the user.
- Decorative details that don't add meaning (the exact shade of the wall, the brand of the coffee cup, unless it's the subject)
- SEO keywords or hashtags — alt text is for accessibility, not search
- Editorializing ("beautiful", "stunning") — describe what makes it beautiful instead, if relevant

## Describing people

This is where alt text most often goes wrong. A few rules of thumb:

- **Don't assume identity from appearance.** Don't assign gender, race, age, or other identity attributes unless they're clearly relevant to the post or the user has told you. When unsure, use neutral phrasing ("a person", "two people") or describe observable features ("someone with short dark hair").
- **Describe expression, not inferred emotion.** "Smiling" is observable; "happy" is an inference. "Hands raised" is observable; "celebrating" is an interpretation (use it if the post context makes it clearly the point, e.g., a graduation photo).
- **Name known people if the post is about them.** If the user is posting a photo of a public figure or someone the post identifies, use the name. Don't guess at identifying strangers.
- **Skip looks-based commentary.** No "attractive", "well-dressed", etc.
- **For the user themselves**: if they say "this is me", you can write "the author" or "me" or just describe the person — match the tone they're using.

## Handling specific image types

**Screenshots of text (tweets, DMs, articles, code):** Transcribe the visible text. If it's a tweet, include the handle and the text. If it's a long article, summarize the key passage and quote the headline. Don't just say "a screenshot of a tweet" — that's the same as no alt text.

**Memes:** Describe the visual setup AND deliver the joke. The format matters ("Drake meme: rejects X, approves Y") plus the actual text. A screen reader user shouldn't have to reconstruct the joke from a generic "two-panel image" description.

**Charts and data viz:** Lead with the takeaway, then the chart type and key data points. "Line chart showing US unemployment falling from 14% in April 2020 to 3.5% by mid-2023" beats "A line chart with an x-axis and y-axis showing data over time."

**Selfies and portraits:** Subject, setting, expression, and anything specifically notable (the outfit if that's the post, the location, what they're holding). Skip the runway-commentary detail.

**Group photos:** Number of people, who they are if named, the occasion. Don't try to describe each person individually unless that's the point of the post.

**Landscape/scenery:** Location if known, key features, light/mood if it's a "look how pretty this is" post. "Golden hour over the Cascades, with Mount Rainier visible above a layer of low clouds" tells a story; "A photograph of mountains" doesn't.

**Food:** What it is, key visible ingredients, presentation if relevant. If it's a recipe post, the dish name matters more than the plating.

**Product photos:** Product name, what it looks like, any visible text/branding. If posting to sell or recommend, include features visible in the image (color, size context).

**Art / illustrations:** Subject, style, medium if discernible, and the mood or what the piece is doing. For abstract work, describe the visual elements (shapes, colors, composition) and the feel.

**Multi-panel (comics, before/after, carousels):** Walk through panel by panel, briefly. "Panel 1: ... Panel 2: ..." or "Left: ... Right: ..."

**Pets and animals:** Species/breed if obvious, what they're doing, anything that makes the photo worth posting (a particularly silly pose, an unusual setting). "Tabby cat asleep in a sink, paws over face" is more useful than "A cat."

## When context is missing

The user usually just hands you an image with no context. That's expected — work with what you can see, and make reasonable inferences about why it's being posted. Default to assuming it's a normal social media share rather than a forensic identification task.

If the image genuinely needs context you can't infer (e.g., it's a photo of someone you don't recognize and the framing suggests they're someone specific, or it's an inside joke where the visual alone doesn't carry the meaning), write the best alt text you can from what's visible and briefly note what additional context from the user would sharpen it. Don't refuse to write alt text just because the meaning isn't fully clear — partial alt text is much better than none.

## Output format

By default, output just the alt text itself — no preamble, no "Here's the alt text:", no quotation marks around it, no markdown. The user is going to copy-paste it into the alt text field. Keep it clean.

If the image has nuances worth flagging (you had to guess at something, you noticed text that's hard to read, there are multiple valid framings depending on what the post says), add a brief note *after* the alt text, separated clearly so it's obvious what's the alt text and what's commentary.

If the user provides the post text or platform, tailor accordingly — match the tone, lean into the angle the post is taking, and respect the platform's character limit.

If the user asks for variations or a different length, provide them without restating everything.

## Examples

**A photo of a golden retriever puppy chewing a sneaker:**
> Golden retriever puppy gnawing on a sneaker, looking up at the camera mid-chew.

(Not: "An adorable golden retriever puppy with fluffy fur is shown chewing on what appears to be a sneaker while looking up at the camera with bright eyes.")

**A screenshot of a tweet from @dril reading "im not owned!! im not owned!!":**
> Tweet from @dril: "im not owned!! im not owned!!"

(Not: "A screenshot of a Twitter post by user dril.")

**A line chart showing rising global temperatures, 1880–2023, with a steep rise after 1980:**
> Line chart of global temperature anomaly from 1880 to 2023. Slow rise until about 1980, then a sharp climb to roughly +1.2°C above the 20th-century average.

**A "Distracted Boyfriend" meme labeled "Me", "My side projects", "Sleep":**
> Distracted Boyfriend meme. The boyfriend (labeled "Me") looks back at a woman ("My side projects") while his girlfriend ("Sleep") glares at him.

**A selfie at a concert, lights and crowd in background:**
> Me at the Phoebe Bridgers show, stage lit purple behind a packed crowd.

(If you don't know it's the user or the artist: "Concert selfie with a packed crowd and purple stage lighting in the background.")

**A photo of a homemade pizza on a wooden board:**
> Homemade pizza with charred crust, fresh basil, and pools of melted mozzarella, on a wooden board.
