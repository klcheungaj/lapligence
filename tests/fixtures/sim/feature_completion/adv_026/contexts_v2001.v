// ADV-026 step 1, Verilog-2001 (IEEE 1364-2001 C.13): $sreadmemh/$sreadmemb
// load a module memory, a static task memory, an automatic task memory, a
// function memory and a memory reached by a hierarchical name. Start and
// finish are arguments, so the load direction does not depend on the edition
// (sim_027 shows the editions differ only where $readmemh omits them).
module sub;
  reg [7:0] hier_mem [3:0];
endmodule

module tb;
  sub u();
  reg [7:0] module_mem [0:3];

  task static_local;
    reg [7:0] cells [5:2];
    begin
      $sreadmemh(cells, 2, 5, "10 11", "12 13");
      $display("static_local[5:2]=%h,%h,%h,%h", cells[5], cells[4], cells[3], cells[2]);
    end
  endtask

  task automatic automatic_local;
    reg [7:0] cells [0:3];
    begin
      $sreadmemh(cells, 3, 0, "20 21", "22 23");
      $display("automatic_local=%h,%h,%h,%h", cells[0], cells[1], cells[2], cells[3]);
    end
  endtask

  function [9:0] function_sum;
    input [7:0] bias;
    reg [7:0] cells [0:3];
    begin
      $sreadmemb(cells, 0, 3, "1 10", "11 100");
      function_sum = bias + cells[0] + cells[1] + cells[2] + cells[3];
    end
  endfunction

  initial begin
    $sreadmemh(module_mem, 0, 3, "30 31", "32 33");
    $display("module=%h,%h,%h,%h", module_mem[0], module_mem[1], module_mem[2], module_mem[3]);
    static_local;
    automatic_local;
    $display("function_sum=%0d", function_sum(8'd100));
    $sreadmemh(u.hier_mem, 0, 3, "10 11", "12 13");
    $display("hierarchical[3:0]=%h,%h,%h,%h", u.hier_mem[3], u.hier_mem[2], u.hier_mem[1], u.hier_mem[0]);
    $finish;
  end
endmodule
