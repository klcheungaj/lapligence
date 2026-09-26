// llg-test-fixture: tests/fixtures/sim/syn032_library_configs/cycle_config.sv
config first;
  design work.cycle_top;
  cell logic_cell use work.second:config;
endconfig
config second;
  design work.cycle_top;
  cell logic_cell use work.first:config;
endconfig
