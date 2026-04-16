import React from 'react';

const features = [
  {
    title: '长按即录',
    description: '按住右 Alt 键说话，松开自动处理。无需切换窗口，无需打开应用。',
    icon: '🎙️',
  },
  {
    title: '双击连续录',
    description: '双击右 Alt 进入连续模式，适合长篇发言。再次单击停止。',
    icon: '♾️',
  },
  {
    title: '本地 + 云端 ASR',
    description: '支持本地 whisper.cpp（无需联网）和 OpenAI Whisper API 两种转写引擎。',
    icon: '🧠',
  },
  {
    title: 'LLM 润色',
    description: '四档润色强度：不润色、修正标点、改善语法、结构化重写。支持 OpenAI / DeepSeek / Anthropic / Ollama。',
    icon: '✨',
  },
  {
    title: '跨平台',
    description: '原生支持 Linux (X11/Wayland)、macOS、Windows，各平台使用最佳系统集成方案。',
    icon: '💻',
  },
  {
    title: '零侵入输出',
    description: '自动写入剪贴板，在任意应用中 Ctrl+V 粘贴。Windows 还支持光标注入。',
    icon: '📋',
  },
];

export default function Cards() {
  return (
    <div style={{
      display: 'grid',
      gridTemplateColumns: 'repeat(auto-fit, minmax(280px, 1fr))',
      gap: '1.5rem',
      marginTop: '2rem',
    }}>
      {features.map((f) => (
        <div key={f.title} style={{
          padding: '1.5rem',
          borderRadius: '12px',
          border: '1px solid var(--ifm-color-emphasis-300)',
          background: 'var(--ifm-background-surface-color)',
        }}>
          <div style={{fontSize: '2rem', marginBottom: '0.5rem'}}>{f.icon}</div>
          <h3 style={{margin: '0 0 0.5rem'}}>{f.title}</h3>
          <p style={{margin: 0, color: 'var(--ifm-color-emphasis-700)', fontSize: '0.95rem'}}>
            {f.description}
          </p>
        </div>
      ))}
    </div>
  );
}
