// llg-test-fixture: SYN-039 selected library/configuration composition.
config syn039_select;
    design work.top;
    cell syn039_cell use gate.gate_select:config;
endconfig
