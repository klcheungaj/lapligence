// llg-test-fixture: tests/fixtures/sim/file_io/deferred_file_output.sv
`timescale 1ns/1ps
module tb;
  integer fd;
  reg [3:0] value;

  initial begin
    fd = $fopen("deferred_file.txt", "w");
    value = 1;
    $fmonitorh(fd, "monitor=%0h", value);
    value = 2;
    $fstrobeb(fd, "strobe=%0b", value);
    #1;
    $fflush(fd);
    $fclose(fd);
    $finish;
  end
endmodule
