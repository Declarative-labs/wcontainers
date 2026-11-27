# The Mesa compiler bridge's headers, as the generator reads them

bridge.h and flat.h are this repository's own C (crates/nir/bridge), copied from the branch `graphics-implementations`
at d19c23829, where GI8 part B builds. The generator's `mesa-compiler` command reads these copies, so that a rerun
writes the same crates; copy them again (and name the commit here) when the bridge changes.
