// llg-test-fixture: tests/fixtures/sim/file_io/file_modes.sv
`timescale 1ns/1ps
module tb;
  integer fd;
  integer status;
  string line;

  initial begin
    fd = $fopen("text_mode.txt", "w");
    $fdisplay(fd, "text");
    $fclose(fd);
    fd = $fopen("binary_mode.txt", "wb");
    $fdisplay(fd, "binary");
    $fclose(fd);

    fd = $fopen("text_mode.txt", "r");
    status = $fgets(line, fd);
    $display("text_as_text=%0d", line.len());
    $fclose(fd);
    fd = $fopen("text_mode.txt", "rb");
    status = $fgets(line, fd);
    $display("text_as_binary=%0d", line.len());
    $fclose(fd);
    fd = $fopen("binary_mode.txt", "r");
    status = $fgets(line, fd);
    $display("binary_as_text=%0d", line.len());
    $fclose(fd);
    $finish(0);
  end
endmodule
