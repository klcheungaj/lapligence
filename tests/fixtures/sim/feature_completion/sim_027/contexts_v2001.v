// SIM-027 A03, Verilog-2001: $readmemh/$readmemb load a module memory, a
// static task memory, an automatic task memory, a function memory and a
// memory reached by a hierarchical name (IEEE 1364-2001 17.2.8). The memory
// argument is the one place 2001 accepts a whole memory.
module sub;
  reg [7:0] hier_mem [3:0];
endmodule

module tb;
  sub u();
  reg [7:0] module_mem [0:3];
  integer f;

  task static_local;
    reg [7:0] cells [5:2];
    begin
      $readmemh("d10.mem", cells);
      $display("static_local[5:2]=%h,%h,%h,%h", cells[5], cells[4], cells[3], cells[2]);
    end
  endtask

  task automatic automatic_local;
    reg [7:0] cells [0:3];
    begin
      $readmemh("d20.mem", cells, 3, 0);
      $display("automatic_local=%h,%h,%h,%h", cells[0], cells[1], cells[2], cells[3]);
    end
  endtask

  function [9:0] function_sum;
    input [7:0] bias;
    reg [7:0] cells [0:3];
    begin
      $readmemb("b.mem", cells);
      function_sum = bias + cells[0] + cells[1] + cells[2] + cells[3];
    end
  endfunction

  initial begin
    f = $fopen("d10.mem");
    $fdisplay(f, "10\n11\n12\n13");
    $fclose(f);
    f = $fopen("d20.mem");
    $fdisplay(f, "20\n21\n22\n23");
    $fclose(f);
    f = $fopen("d30.mem");
    $fdisplay(f, "30\n31\n32\n33");
    $fclose(f);
    f = $fopen("b.mem");
    $fdisplay(f, "1\n10\n11\n100");
    $fclose(f);
    $readmemh("d30.mem", module_mem);
    $display("module=%h,%h,%h,%h", module_mem[0], module_mem[1], module_mem[2], module_mem[3]);
    static_local;
    automatic_local;
    $display("function_sum=%0d", function_sum(8'd100));
    $readmemh("d10.mem", u.hier_mem);
    $display("hierarchical[3:0]=%h,%h,%h,%h", u.hier_mem[3], u.hier_mem[2], u.hier_mem[1], u.hier_mem[0]);
    $finish;
  end
endmodule
