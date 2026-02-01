# README Translation Plan

This document outlines the strategy for internationalizing OMG's README and documentation.

---

## 🌍 Target Languages (Priority Order)

### Tier 1 (High Priority - Large Developer Communities)
1. **🇨🇳 Chinese (Simplified)** - `README.zh-CN.md`
   - Large Arch Linux community in China
   - Strong developer presence
   - GitHub shows 30%+ Chinese developers

2. **🇯🇵 Japanese** - `README.ja.md`
   - Strong tech adoption
   - High-quality developer tools culture
   - Growing Rust community

3. **🇰🇷 Korean** - `README.ko.md`
   - Active developer community
   - High GitHub activity
   - Strong interest in dev tools

### Tier 2 (Medium Priority - Growing Communities)
4. **🇪🇸 Spanish** - `README.es.md`
   - 2nd most spoken language globally
   - Growing Latin American dev community

5. **🇫🇷 French** - `README.fr.md`
   - Strong Francophone developer community
   - Europe + Africa presence

6. **🇩🇪 German** - `README.de.md`
   - Strong European developer community
   - High-quality tool adoption

### Tier 3 (Nice to Have)
7. **🇧🇷 Portuguese (Brazilian)** - `README.pt-BR.md`
8. **🇷🇺 Russian** - `README.ru.md`
9. **🇮🇳 Hindi** - `README.hi.md`

---

## 📋 Translation Scope

### What to Translate

**README.md (Full Translation):**
- All sections from English README
- Preserve code examples (don't translate commands)
- Translate comments in code examples
- Adapt cultural references if needed

**Quick Start (Quick Translation):**
- `docs/quickstart.md` → `docs/quickstart.[lang].md`
- Essential for onboarding

**FAQ (High Value):**
- `docs/faq.md` → `docs/faq.[lang].md`
- Common questions should be accessible

### What NOT to Translate

**Technical Content (Keep in English):**
- CLI commands and flags
- Configuration file examples
- Error messages (match actual output)
- File/directory names
- Code snippets (only translate comments)

**Advanced Docs (English Only for Now):**
- Architecture docs
- API reference
- Enterprise features
- Developer guides

---

## 🏗️ Implementation Strategy

### Option 1: Manual Translation (Recommended for Quality)

**Pros:**
- Highest quality
- Cultural adaptation
- Technical accuracy

**Cons:**
- Time-consuming
- Requires native speakers
- Maintenance overhead

**Process:**
1. Create template structure
2. Recruit community translators (GitHub Discussions)
3. Review process (native speaker + maintainer)
4. Ongoing sync with English version

### Option 2: Machine Translation + Human Review

**Pros:**
- Fast initial translation
- Lower cost
- Good baseline

**Cons:**
- Technical terms may be wrong
- Awkward phrasing
- Still needs review

**Process:**
1. Use GPT-4/DeepL for initial translation
2. Technical review by native speaker
3. Cultural adaptation
4. Community feedback

### Option 3: Hybrid (Recommended)

**Combine both approaches:**
1. Machine translation for first draft (GPT-4)
2. Community review and refinement
3. Maintainer approval
4. Continuous improvement via issues

---

## 📁 File Structure

```
omg/
├── README.md                    # English (canonical)
├── README.zh-CN.md              # Chinese (Simplified)
├── README.ja.md                 # Japanese
├── README.ko.md                 # Korean
├── README.es.md                 # Spanish
├── README.fr.md                 # French
├── README.de.md                 # German
├── README.pt-BR.md              # Portuguese (Brazilian)
├── README.ru.md                 # Russian
├── README.hi.md                 # Hindi
│
├── docs/
│   ├── i18n/
│   │   ├── zh-CN/
│   │   │   ├── quickstart.md
│   │   │   ├── faq.md
│   │   │   └── installation.md
│   │   ├── ja/
│   │   │   ├── quickstart.md
│   │   │   ├── faq.md
│   │   │   └── installation.md
│   │   └── ko/
│   │       ├── quickstart.md
│   │       ├── faq.md
│   │       └── installation.md
│   └── ...
```

---

## 🔄 Synchronization Strategy

### Version Tracking

Add metadata to translated files:

```markdown
---
original: README.md
original_hash: a1b2c3d4e5f6
translation_date: 2026-02-01
translator: @username
reviewer: @reviewer
status: up-to-date  # or "needs-update", "outdated"
---
```

### Update Detection

**Automated Approach:**
```bash
# GitHub Action to detect changes
name: Check Translation Sync
on:
  push:
    paths:
      - 'README.md'
      - 'docs/quickstart.md'
      - 'docs/faq.md'

jobs:
  check-translations:
    runs-on: ubuntu-latest
    steps:
      - name: Check if translations need update
        run: |
          # Compare hash of English file with translated file metadata
          # Create issue if out of sync
```

**Manual Approach:**
- Quarterly review of translations
- GitHub issues for updates needed
- Community notifications

---

## 🎯 Translation Template

### Header Section (All Languages)

Add language selector at the top of each translated README:

```markdown
# OMG

[English](README.md) | [简体中文](README.zh-CN.md) | [日本語](README.ja.md) | [한국어](README.ko.md) | [Español](README.es.md) | [Français](README.fr.md) | [Deutsch](README.de.md)

---

**Stop switching between 7 package managers.**

[Badges remain in English]

[Translated description...]
```

---

## 👥 Community Translation Process

### 1. Call for Translators

**GitHub Discussion Template:**

```markdown
# 🌍 Call for Translators

We're internationalizing OMG documentation! We need native speakers to help translate:

**High Priority:**
- [ ] 🇨🇳 Chinese (Simplified)
- [ ] 🇯🇵 Japanese
- [ ] 🇰🇷 Korean

**Medium Priority:**
- [ ] 🇪🇸 Spanish
- [ ] 🇫🇷 French
- [ ] 🇩🇪 German

**What to translate:**
- README.md (~500 lines)
- docs/quickstart.md (~700 lines)
- docs/faq.md (~400 lines)

**Requirements:**
- Native speaker
- Technical background (familiar with package managers, CLI tools)
- Time commitment: 4-8 hours

**Credit:**
- Listed in CONTRIBUTORS.md
- Mentioned in release notes
- GitHub contributor badge

**Interested?** Comment below with:
1. Language you can translate
2. Your background (developer, translator, both)
3. Availability (hours/week)
```

### 2. Translation Workflow

```
1. Claim language → Comment on GitHub Discussion
2. Fork repository → Create translation branch
3. Translate → Use template, preserve formatting
4. Submit PR → Tag maintainers + community reviewer
5. Review → Native speaker review + maintainer approval
6. Merge → Credit in release notes
7. Maintain → Periodic updates as English version changes
```

### 3. Quality Guidelines

**For Translators:**
- Preserve all links, code examples, and technical terms
- Adapt cultural references (if any)
- Use consistent terminology
- Test all links in translated version
- Keep formatting identical to English version

**For Reviewers:**
- Native speaker proficiency required
- Technical accuracy check
- Cultural appropriateness
- Consistency with established terminology

---

## 🛠️ Technical Implementation

### Initial Translation Script

```bash
#!/bin/bash
# scripts/translate-readme.sh

LANG=$1  # e.g., "zh-CN"

if [ -z "$LANG" ]; then
  echo "Usage: ./scripts/translate-readme.sh <language-code>"
  echo "Example: ./scripts/translate-readme.sh zh-CN"
  exit 1
fi

# Create translation file
OUTPUT="README.$LANG.md"

# Add language selector header
cat > "$OUTPUT" <<EOF
# OMG

[English](README.md) | [简体中文](README.zh-CN.md) | [日本語](README.ja.md) | [한국어](README.ko.md) | [Español](README.es.md) | [Français](README.fr.md) | [Deutsch](README.de.md)

---

<!-- Translation metadata -->
<!--
original: README.md
translation_date: $(date +%Y-%m-%d)
status: draft
-->

EOF

# Append machine-translated content (requires GPT-4 API key)
# (This is a placeholder - actual implementation would call translation API)

echo "Machine translation placeholder for $LANG"
echo "Next steps:"
echo "1. Review and refine translation in $OUTPUT"
echo "2. Test all links and commands"
echo "3. Submit PR for community review"
```

### Link Localization

Some links may need localization:

```markdown
<!-- English -->
[Full Documentation](https://pyro1121.com/docs)

<!-- Japanese -->
[完全なドキュメント](https://pyro1121.com/ja/docs)

<!-- If no localized docs yet, keep English link -->
[完全なドキュメント](https://pyro1121.com/docs)
```

---

## 📊 Translation Progress Tracking

### GitHub Project Board

Create "Translations" project with columns:
- **To Translate** - Languages planned
- **In Progress** - Active translation work
- **Review** - Needs native speaker review
- **Approved** - Reviewed and merged
- **Needs Update** - Out of sync with English version

### Progress Badge

Add to README.md:

```markdown
## 🌍 Available Languages

[![zh-CN](https://img.shields.io/badge/简体中文-完成-green)](README.zh-CN.md)
[![ja](https://img.shields.io/badge/日本語-进行中-yellow)](README.ja.md)
[![ko](https://img.shields.io/badge/한국어-计划中-lightgrey)](README.ko.md)
```

---

## 🎯 Success Metrics

Track effectiveness of translations:

1. **GitHub Traffic by Country**
   - Monitor traffic from target language countries
   - Measure increase after translation launch

2. **GitHub Stars by Region**
   - Track star growth from translated regions
   - Compare before/after translation

3. **Issue Activity**
   - Monitor issues/discussions in translated languages
   - Engagement from new communities

4. **Translation Freshness**
   - % of translations up-to-date vs outdated
   - Target: >90% current

---

## 🚀 Rollout Plan

### Phase 1: Foundation (Week 1)
- [ ] Create file structure
- [ ] Set up translation workflow
- [ ] Create contributor guidelines
- [ ] Post call for translators

### Phase 2: Tier 1 Languages (Weeks 2-4)
- [ ] Chinese (Simplified)
- [ ] Japanese
- [ ] Korean
- [ ] Community review process

### Phase 3: Tier 2 Languages (Weeks 5-8)
- [ ] Spanish
- [ ] French
- [ ] German

### Phase 4: Maintenance (Ongoing)
- [ ] Quarterly sync checks
- [ ] Update translations as English changes
- [ ] Community feedback incorporation

---

## 📝 Translation Glossary

Create consistent terminology across languages:

| English | 简体中文 | 日本語 | 한국어 | Español | Français | Deutsch |
|---------|---------|--------|--------|---------|----------|---------|
| Package Manager | 包管理器 | パッケージマネージャー | 패키지 관리자 | Gestor de Paquetes | Gestionnaire de Paquets | Paketmanager |
| Runtime | 运行时 | ランタイム | 런타임 | Tiempo de Ejecución | Environnement d'Exécution | Laufzeitumgebung |
| Security Scan | 安全扫描 | セキュリティスキャン | 보안 스캔 | Escaneo de Seguridad | Analyse de Sécurité | Sicherheitsscan |
| Daemon | 守护进程 | デーモン | 데몬 | Demonio | Démon | Daemon |
| Lock File | 锁定文件 | ロックファイル | 잠금 파일 | Archivo de Bloqueo | Fichier de Verrouillage | Sperrdatei |

(Expand as needed with community input)

---

## 🔗 Resources

**Translation Tools:**
- DeepL: https://www.deepl.com (better than Google for technical content)
- GPT-4: For initial drafts
- GitHub Discussions: Community collaboration

**Style Guides:**
- Microsoft Terminology: https://www.microsoft.com/en-us/language
- Google Developer Documentation Style Guide (multilingual)

**Community:**
- GitHub Discussions: Coordinate translation efforts
- Discord/Slack: Real-time translator coordination (if created)

---

## 🙏 Acknowledgments

All translators will be credited in:
- CONTRIBUTORS.md
- Release notes
- Translated README header
- GitHub contributor graph

---

**Ready to start?** See [CONTRIBUTING.md](../CONTRIBUTING.md) for translation guidelines.
