// Decision S27-D12: a $writememh call whose start or finish address lies
// outside the array is rejected before the file is opened, so an existing
// file keeps its contents.
//
// IEEE 1800-2009 21.5 (SystemVerilog-1800-2009.txt L37472-37473):
//   "If filename exists at the time $writememb or $writememh is called, the
//   file will be overwritten (i.e., there is no append mode)."
//
// The text does not cover a call whose range is invalid; llg reports an
// error and leaves the file alone.
module tb;
  logic [7:0] mem [0:3];
  logic [7:0] back [0:1];
  integer fd;
  initial begin
    fd = $fopen("s27_d12.mem", "w");
    $fwrite(fd, "5a\na5\n");
    $fclose(fd);
    mem = '{8'h00, 8'h11, 8'h22, 8'h33};
    $writememh("s27_d12.mem", mem, 2, 9);
    $readmemh("s27_d12.mem", back);
    $display("file %h %h", back[0], back[1]);
    $finish;
  end
endmodule
