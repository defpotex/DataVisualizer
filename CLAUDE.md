DataVisualizer Application

Goal: Production-Quality Engineering Application to display vehicle data on a map, with user extensible plotting. Similar to Tableau, but for live/replay vehicle test data.

Tech Stack: Rust + egui/eframe (GUI) + polars (data engine) + walkers (maps) + egui_plot (charts) + crossbeam-channel (threading) + rfd (file dialogs) + serde/zip (session persistence)

Design documentation in docs/ folder.
- High Level Product Requirements: docs/requirements.md
- System Design & Architecture:    docs/architecture.md
- Priority-ordered Feature Roadmap: docs/roadmap.md

Current Build Phase: Phase 3 ✅ complete. Phase 4 (CSV Data Loading) is next.


Behavior Requirements:
- Adhere to all requirements in requirements.md
- Build the application one step at a time, based on roadmap.md
    - Keep and outline and short notes on each stage of the roadmap. 
    - Roadmap should be organized functionally (per feature, hierarchically), and separately by priority.
    - Functional and Priority listings should be synched, maintained, statused, and updated with each code change, as applicable.
- Keep architecture documentation up to date - update architecture.md with each code change as needed
- Maintin rich architecture documentation. Flow diagrams, why decisions were made, etc. Tie these decisions to the requirement(s) that drove them
- Protect main branch, work in feature branches and merge to main when prompted.
- For each feature:
    - Plan First - outline the approach before writing code. I will review this before proceeding.
    - Implement
    - Test - run your own tests. I will run usage tests when ready.
    - Update Documentation - Update documentation to align with changes. Include a changelog.
    - Commit - Add good comments and commit
    - Do not move to the next feature until the current one is committed and is approved by me.

