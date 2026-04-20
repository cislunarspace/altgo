# Overlay Animation & Settings Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Improve overlay (floating window) animations to be faster and more responsive, and redesign the Settings page with PyQt-style functional layout including model management.

**Architecture:**
- Overlay animations handled purely in CSS (`overlay.css`) with updated keyframes and timing
- Settings page uses new vertical form layout with compact PyQt-style CSS
- State transitions in `overlay.tsx` use opacity crossfade between recording/processing/done states

**Tech Stack:** React (Tauri frontend), CSS animations, TypeScript

---

## Part 1: Overlay Animation

### Task 1: Update overlay.css animation timings and keyframes

**Files:**
- Modify: `frontend/src/overlay.css`

**Changes:**
- Change `.island` entry animation: duration 400ms → 200ms, delay 400ms → 120ms, easing `cubic-bezier(0.34, 1.56, 0.64, 1)` → `cubic-bezier(0.16, 1, 0.3, 1)`, transform from `scale(0.5) translateY(20px)` to just `translateY(8px)`
- Change `.island-result` entry animation: same timing and easing as `.island`
- Add exit animation `.island-exit`: duration 150ms, linear, fade out only (no scale)
- Change `pulse-recording` keyframe: pulse duration 1.2s → 0.8s, scale range 1.0→1.1 → 1.0→1.05
- Change `pulse-ring` keyframe: scale 2.5 → 2.0

### Task 2: Add state transition crossfade in overlay.tsx

**Files:**
- Modify: `frontend/src/overlay.tsx`

**Changes:**
- Add CSS class `.island-exit` with 150ms linear fade-out animation
- When status changes between recording/processing/done, apply exit animation first (150ms), then switch content
- OR: use two overlapping elements with opacity crossfade (100ms) during state transitions
- Recording indicator pulse should use new 0.8s timing from CSS

### Task 3: Update StatusIndicator component pulse timing

**Files:**
- Modify: `frontend/src/components/StatusIndicator.tsx`
- Modify: `frontend/src/styles/components.css`

**Changes:**
- In `StatusIndicator.tsx`: change pulse interval from 1500ms to 800ms, pulse scale from 1.15 to 1.05
- In `components.css`: update pulse-recording keyframe to 0.8s duration, 1.0→1.05 scale

---

## Part 2: Settings Page (PyQt Style)

### Task 4: Add PyQt-style CSS to components.css

**Files:**
- Modify: `frontend/src/styles/components.css`

**Changes:**
Add new CSS classes for PyQt-style settings form (insert after existing settings styles):

```css
/* PyQt-style Settings Form */
.settings-form {
  background: #1a1a1a;
  padding: 16px;
}

.settings-form-section {
  padding: 16px 0;
  border-bottom: 1px solid #333;
}

.settings-form-section:last-child {
  border-bottom: none;
}

.settings-form-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 0;
  gap: 16px;
}

.settings-form-label {
  font-size: 13px;
  color: #ccc;
  flex-shrink: 0;
}

.settings-form-control {
  flex: 1;
  max-width: 280px;
  display: flex;
  justify-content: flex-end;
}

.settings-form-input {
  width: 100%;
  padding: 6px 10px;
  background: #252525;
  border: 1px solid #444;
  border-radius: 2px;
  color: #fff;
  font-size: 13px;
  font-family: inherit;
  transition: border-color 0.15s;
}

.settings-form-input:focus {
  outline: none;
  border-color: #6366f1;
}

.settings-form-input:hover {
  border-color: #555;
}

.settings-form-input:disabled {
  background: #1a1a1a;
  color: #555;
}

.settings-form-select {
  width: 100%;
  padding: 6px 10px;
  background: #252525;
  border: 1px solid #444;
  border-radius: 2px;
  color: #fff;
  font-size: 13px;
  font-family: inherit;
  cursor: pointer;
  transition: border-color 0.15s;
  appearance: none;
  background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='10' height='10' viewBox='0 0 24 24' fill='none' stroke='%23999' stroke-width='2'%3E%3Cpath d='m6 9 6 6 6-6'/%3E%3C/svg%3E");
  background-repeat: no-repeat;
  background-position: right 8px center;
  padding-right: 28px;
}

.settings-form-select:focus {
  outline: none;
  border-color: #6366f1;
}

.settings-form-btn {
  padding: 6px 16px;
  background: #333;
  border: 1px solid #444;
  border-radius: 2px;
  color: #ccc;
  font-size: 13px;
  font-family: inherit;
  cursor: pointer;
  transition: all 0.15s;
}

.settings-form-btn:hover {
  background: #444;
  border-color: #555;
}

.settings-form-btn:focus {
  outline: none;
  border-color: #6366f1;
}

.settings-form-btn:disabled {
  background: #222;
  color: #666;
  cursor: not-allowed;
}

.settings-form-btn-primary {
  background: #6366f1;
  border-color: #6366f1;
  color: #fff;
}

.settings-form-btn-primary:hover {
  background: #818cf8;
  border-color: #818cf8;
}

.settings-form-checkbox-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.settings-form-checkbox {
  width: 16px;
  height: 16px;
  accent-color: #6366f1;
  cursor: pointer;
}
```

### Task 5: Rewrite Settings.tsx with PyQt-style vertical form layout

**Files:**
- Modify: `frontend/src/pages/Settings.tsx`
- Add i18n keys for model management section

**Changes:**
Rewrite Settings.tsx to use the new PyQt-style form layout. Remove Card components, use plain form rows instead.

**Section 1 - Basic Settings:**
- Language (select: zh/en)

**Section 2 - Recording:**
- Key name (text input)

**Section 3 - Transcription:**
- Engine (select: api/local)
- Language (text input, shown for both)
- Model/URL (text input, changes label based on engine: "Model" for api, "Model Path" for local)
- API Key (password input, only shown when engine=api)

**Section 4 - Model Management (new):**
- Current model display (text, read-only)
- Model path (text input for local engine)
- Download model button (triggers download)
- Delete model button (with confirmation)

**Section 5 - Polishing:**
- Polish level (select: none/light/medium/heavy)
- API URL (text input)
- Model (text input)
- API Key (password input)

**Section 6 - About:**
- Version display
- Check updates button

Keep existing save/cancel logic. Preserve existing i18n keys where they exist, add new ones for model management.

### Task 6: Add i18n keys for model management

**Files:**
- Modify: `frontend/src/i18n/index.ts`

**Changes:**
Add these keys to both zh and en translation objects:
- `settings.model_management`: "模型管理" / "Model Management"
- `settings.current_model`: "当前模型" / "Current Model"
- `settings.download_model`: "下载模型" / "Download Model"
- `settings.delete_model`: "删除模型" / "Delete Model"
- `settings.model_downloaded`: "已下载" / "Downloaded"
- `settings.model_not_downloaded`: "未下载" / "Not Downloaded"
- `settings.confirm_delete`: "确认删除？" / "Confirm Delete?"
- `settings.no_model_configured`: "未配置" / "Not configured"

---

## Spec Reference

Full spec: `docs/superpowers/specs/2026-04-18-overlay-animation-design.md`
