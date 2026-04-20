# Overlay & Settings Animation Redesign

## Status

Approved: 2026-04-18

## Overview

Improve two areas:
1. **Overlay (floating window)** — faster, more responsive animations
2. **Settings page** — PyQt-style functional layout with model management

---

## Part 1: Overlay Animation

### Design Principles

- **Fast response** — appear in ≤200ms after Alt key press
- **Light and smooth** — no bounce, no heavy scaling
- **State clarity** — clear visual feedback for recording/processing/done states

### Animation Specifications

#### Entry Animation

| Property | Before | After |
|----------|--------|-------|
| Delay | 400ms | 120ms |
| Duration | 400ms | 200ms |
| Easing | `cubic-bezier(0.34, 1.56, 0.64, 1)` (spring) | `cubic-bezier(0.16, 1, 0.3, 1)` (quick out) |
| Transform | scale(0.5) + translateY(20px) | translateY(8px) |
| Opacity | 0 → 1 | 0 → 1 |

#### Exit Animation

| Property | Before | After |
|----------|--------|-------|
| Duration | 400ms | 150ms |
| Easing | same as entry | linear |
| Effect | scale down + fade | fade out only |

#### State Transitions

- **Recording → Processing**: crossfade, 100ms
- **Processing → Done**: crossfade, 100ms
- No scale transforms during state changes

#### Recording Indicator

| Property | Before | After |
|----------|--------|-------|
| Pulse duration | 1.2s | 0.8s |
| Scale range | 1.0 → 1.1 | 1.0 → 1.05 |
| Ring expansion | scale 2.5 | scale 2.0 |

---

## Part 2: Settings Page (PyQt Style)

### Layout Principles

- **Vertical single-column** layout
- **Classic form layout**: label on left, control on right
- **Functional density**: compact spacing, clear section separation
- **No decorative elements**: no gradients, no glass effects, no rounded cards

### Visual Specifications

| Property | Value |
|----------|-------|
| Background | `#1a1a1a` (dark gray) |
| Section divider | 1px solid `#333` |
| Label color | `#ccc` |
| Input background | `#252525` |
| Input border | 1px solid `#444` |
| Focus border | `#6366f1` |
| Font size | 13px |
| Section padding | 16px vertical |
| Field spacing | 12px between fields |

### Sections

#### 1. Basic Settings
- API Key (password input)
- Language selector
- Startup on login (checkbox)

#### 2. Transcription Settings
- Model selection (dropdown: whisper-api / local-whisper)
- Polisher level (dropdown: none / light / medium / heavy)
- Polisher API key (password input)

#### 3. Model Management (new)
- Current model display
- Download button → opens model download dialog
- Delete model button
- Download progress bar
- Model path selector

#### 4. About
- Version number
- Check for updates button

### Component States

| Component | Normal | Hover | Focus | Disabled |
|-----------|--------|-------|-------|----------|
| Input | #252525 bg, #444 border | #333 border | #6366f1 border | #1a1a1a bg, #555 text |
| Button | #333 bg, #ccc text | #444 bg | #6366f1 border | #222 bg, #666 text |
| Dropdown | same as input | same as input | same as input | — |

---

## Implementation Notes

### CSS Variables to Update

```css
/* overlay.css */
.island {
  animation: island-in 0.2s cubic-bezier(0.16, 1, 0.3, 1) 0.12s both;
}

@keyframes island-in {
  from {
    transform: translateY(8px);
    opacity: 0;
  }
  to {
    transform: translateY(0);
    opacity: 1;
  }
}
```

### Performance Targets

- First paint: ≤100ms after key press
- Animation jank: 0 dropped frames
- Settings page load: ≤50ms
