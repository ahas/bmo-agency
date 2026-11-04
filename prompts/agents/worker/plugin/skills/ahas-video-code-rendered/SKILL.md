---
name: "ahas-video-code-rendered"
bundle: worker
version: "1.0.0"
description: "How to make a video as code, in which every frame is a pure function of time: a treatment and style bible first, timing data measured from the audio, one module per scene, an agent that looks at its own frames, a contact-sheet review loop, parallel scene authors under a written guide, and an offline render with motion blur. Read beside ahas-video-shotcraft for any video that should look authored rather than templated."
---

# Making a video as code

The method drawn from the open source of a widely seen music video made by a coding agent in conversation with its owner (`mexicat/pdoom-video`, MIT), its fork in a painted style (`JohnHeibel/PDoomVideo`) and an agent kit built on that (`Kreycin/pdoom-mv-kit`, MIT). This is the method only. Their song belongs to its authors and is not ours to use; their scenes, characters and designs are not to be copied. Take the discipline, make your own picture.

## What made it work

1. **Every frame is a pure function of time.** Any second can be drawn at once, a fix is re-rendered alone, frames render in parallel and out of order, and the preview equals the export. No state carried between frames, no `Math.random()`, no clock. Randomness is seeded; per-frame jitter is keyed to the frame's index.
2. **The agent looks at what it renders.** Animation cannot be judged from code. After every change it renders stills or a contact sheet and opens the images. A worker that cannot view images says so at once.
3. **A written treatment before any scene:** one paragraph of idea, the tone, what is forbidden, the palette, the type, the recurring motifs, then the plates one by one. Every later decision answers to it.
4. **Timing is measured, not guessed.** Beats, downbeats, sections, onsets and envelopes come from analysing the actual audio into a JSON file; word timings come from alignment against the actual narration. Scenes read that data, and find a line by its content, never by a hard-coded second.
5. **One scene, one module, one idea.** A scene owns its look; a timeline file owns the edit. Scene authors never touch the timeline or the engine.
6. **Revisions are named and specific,** each listing what changed and why, from watching: "this flash is too evident, carry the hit with a zoom punch", "this transition is a cut, make the line continue across it".

## The treatment (write it first, in the project folder)

- **The idea in one paragraph:** one conceit that holds the whole film together. A film of disconnected effects has none.
- **Tone, in words you can test a frame against:** what it is and what it is not.
- **The forbidden list:** the generic looks, by name. For anything about AI: no neon purple and cyan, no glowing brains, no code rain, no lens-flare soup, no particle nebulae, no stock imagery, no mascots unless the brief asks. If a frame could be anyone's, it fails.
- **Palette:** a few named values and one signal colour that alone may glow. One rare accent owned by one moment. Light and dark plates alternate for rhythm.
- **Type:** two or three families with a stated role each. Proper kerning and typographic punctuation. Swiss-grid discipline, asymmetry, negative space, hairlines and small annotations beside big display type.
- **Motifs:** a few things that recur and change, so the film builds: one number that climbs, one mark that returns, one template that escalates at each return.
- **Humour or gravity by detail:** deadpan captions, tiny footnotes, a form stamped. Detail is what makes it look authored.

## Motion and editing

- Something always moves, and big changes land on the beat: cuts on downbeats, hits on kicks and snares, camera moves that ease into a downbeat.
- Strong easing, anticipation and overshoot. No floating screensaver motion. Inside a scene there are sub-cuts, reframings and snaps.
- **Every scene hands on to the next.** The action carries across the cut: a line that continues, a zoom through an object, a page that opens onto the next picture. A plain cut or crossfade is the exception and is chosen.
- One clear focal action per shot and a silhouette that reads at thumbnail size. Shots of 1.4 to 4 seconds, unless holding is the point. A gesture needs 0.4 s to read, a reaction 0.3 s, a reveal 0.8 s.
- Something happens in every shot, and something answers every action.
- Flashes are rare: carry most hits with a zoom punch or a colour fringe and keep the frame's blacks.

## Words on screen

- Each line is integrated graphically and differently from the last: typed, stamped, plotted, drawn along a curve. Words are part of the image, not subtitles laid over it.
- Synced per word to its measured start. Anticipation is allowed; highlighting never runs ahead of the voice.
- Inside the title-safe area, and readable.

## The review loop

Keep a render tool with these modes, and use them in this order:

| Mode | Use |
|---|---|
| stills at given times | the first and last frame of a shot, and a moment |
| contact sheet of a range, about a frame per beat | does each shot read, does the whole move |
| strip of every frame of a key move or seam | popping, double exposure, a seam that does not carry |
| crop at full size | faces, type, hairlines |
| short clip with sound | do hits land against the audio |
| one frame every 4 s of the whole film | the shape of the film, its light and dark rhythm |

Check: is any line unreadable, anything cropped, frozen, popping or washed out by glow, does every seam carry, do hits land on beats. Fix, re-render only what changed, look again.

## Working with parallel authors

A lead writes the treatment, the engine and a guide for scene authors, then gives each subagent one scene. The guide states the API, the frame's coordinate rules, the data available, the check commands, and: "only edit your own scene file; report what you need from the engine". Keep a progress file with one line per scene (written, checked, fixed) so a new session can continue.

## The render

- Offline, in headless Chrome, with the app taking a time and returning a frame; raw frames streamed to one ffmpeg, in order, by several browsers rendering chunks.
- **Motion blur:** average sub-frames over a short shutter (a fifth of the frame time works), adaptively, more where the picture is still changing. Then a fast whip leaves a streak rather than stepped copies. This alone lifts the look far above a plain Remotion render. Grain and post come after the average.
- Tag the stream BT.709 and name the colour range. Very fine grain is expensive to encode; set the rate by looking at the file size.
- Rendering on this platform takes a card ticket where the producer's rule says so; see `ahas-video-shotcraft`.

## Rights

Music, footage and images need a licence you can name, recorded in `NOTES.md`. Do not download music from a video site; an original track made here (or one the owner licensed) is the road. Do not draw a character from a show, a meme's drawing or another video's design, even from memory: design your own. Name a product only where the brief does.

## Checking the result

Duration equals the audio's, the streams are as asked, and a one-frame-per-4-s sheet of the whole film has been looked at. Report where the file is, how long the render took, and whatever of the treatment you could not do.
