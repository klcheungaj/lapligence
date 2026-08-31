// llg-lsp-fixture: workspace/unrelated.sv
module unrelated(input logic pin);
  logic [7:0] internal_bus;
  function automatic logic make_value(input logic arg);
    logic function_local;
    make_value = function_local ^ arg;
  endfunction
  task automatic drive_value(input logic arg);
    logic task_local;
    task_local = arg;
  endtask
endmodule
