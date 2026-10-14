// Decision S27-D13: the memory_name argument of $readmemh/$writememh is
// evaluated once per call. A selector with side effects in it (here a
// function call that counts its calls) runs once, for a load, which reads
// and stores the selected memory, and for a dump.
//
// IEEE 1800-2009 21.4 (SystemVerilog-1800-2009.txt L37251-37252):
//   "The memory_name can be an unpacked array or a partially indexed
//   multidimensional unpacked array that resolves to a lesser dimensioned
//   unpacked array (see 21.4.3)."
// IEEE 1800-2009 11.4.1 (L14478-14479), the nearest rule for a target that
// is both read and written:
//   "An assignment operator is semantically equivalent to a blocking
//   assignment, with the exception that any left-hand index expression is
//   only evaluated once."
//
// The text does not say how often memory_name is evaluated; llg evaluates it
// once, as it does every other task argument.
module tb;
  typedef struct { logic [7:0] cells [0:3]; } record_t;
  int calls;
  logic [7:0] back [0:3];

  function automatic int next();
    calls++;
    return calls - 1;
  endfunction

  task automatic run;
    record_t records [0:1];
    logic [7:0] grid [0:1][0:3];
    calls = 0;
    $readmemh("s27_d13.mem", records[next()].cells);
    $display("load calls=%0d %h %h", calls, records[0].cells[0], records[0].cells[3]);
    calls = 0;
    $writememh("s27_d13_dump.mem", records[next()].cells);
    $readmemh("s27_d13_dump.mem", back);
    $display("dump calls=%0d %h %h", calls, back[0], back[3]);
    calls = 1;
    $readmemh("s27_d13.mem", grid[next()]);
    $display("row calls=%0d %h %h", calls, grid[1][0], grid[1][3]);
  endtask

  initial begin
    integer fd;
    fd = $fopen("s27_d13.mem", "w");
    $fwrite(fd, "11\n22\n33\n44\n");
    $fclose(fd);
    run();
    $finish;
  end
endmodule
