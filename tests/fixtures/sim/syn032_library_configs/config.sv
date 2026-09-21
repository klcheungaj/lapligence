// llg-test-fixture: tests/fixtures/sim/syn032_library_configs/config.sv
config choose;
  design work.top;
  default liblist rtl;
  cell logic_cell use gate.gate_cfg:config;
  instance top.from_instance use rtl.logic_cell;
endconfig
