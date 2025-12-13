---
version: "1.0"
lastUpdated: "2025-12-09"
lifecycle: core
stakeholder: pknull
changeTrigger: "scope change, objectives shift"
validatedBy: "project owner"
dependencies: []
---

# Project Brief: termart

## Overview

Terminal art generator creating animated ASCII/Unicode visualizations. Serves as both screensaver and interactive art application.

## Core Components

1. **Bonsai Trees**: Procedurally generated ASCII bonsai with customizable growth parameters and animation
2. **Visualizations**: Nine animated visual effects (matrix, life, plasma, fire, rain, waves, cube, pipes, donut)

## Objectives

- Provide visually appealing terminal-based art and screensaver functionality
- Support interactive real-time speed and color scheme controls
- Enable reproducible generation via seeds
- Maintain smooth rendering through buffer abstraction

## Constraints

- Terminal-only rendering (no GUI dependencies)
- Must handle terminal resize gracefully
- Cross-platform where crossterm supports
- Single binary, minimal dependencies

## Success Criteria

- Smooth 60fps-capable rendering
- All visualizations interactive with consistent controls
- Clean exit restoring terminal state
- Intuitive CLI interface
