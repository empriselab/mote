# Contributing to `mote-hardware`

## ECAD 

`mote-hardware` contains the KiCAD v9.0 project files for the Mote circuit board.

Hardware changes are tested on pull request [in CI](https://github.com/empriselab/mote-core/blob/main/.github/workflows/hardware-check.yaml).
Manufacturing files are released on any tag to `mote-core` matching the pattern `mote-hardware-vX.X.X`.

ECAD files are difficult to source control. If you would like to make a contribution to `mote-hardware` please announce your intention and timeline via [the issue tracker](https://github.com/empriselab/mote/issues).
This helps prevent simultaneous branches that are impossible to merge.

## CAD

The PCB outline and 3D printed parts are designed in OnShape. You can copy [the OnShape workspace](https://cad.onshape.com/documents/1587a90b12bee427526d37dc/w/ed0eeed14d699ca63f4b9c5c/e/4450af53acbcf927be21561b?renderMode=0&uiState=6a4c555eb68fc45f556fa0f5) if you would like to make modifications.

To have your changes incorporated into the main design, please [create an issue](https://github.com/empriselab/mote/issues) describing your modifications and linking to your copy of the workspace.
