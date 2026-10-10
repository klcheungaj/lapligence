// Decision S26-D11: a successful $fseek or $rewind clears the error status
// and the end-of-file indicator of the descriptor.
//
// IEEE 1800-2009 21.3.7 (SystemVerilog-1800-2009.txt L37203-37205):
//   "The integral value of the error code is returned in errno. If the most
//   recent operation did not result in an error, then the value returned
//   shall be zero, and the str variable shall be cleared."
//
// llg keeps the status of the last failing operation until a successful
// reposition, which clears it (as C clearerr). The read of a write-only file
// fails; the following $rewind succeeds.
module tb;
  integer fd, g, e1, e2, r;
  string msg;
  initial begin
    fd = $fopen("s26_d11.txt", "w");
    $fwrite(fd, "data\n");
    g = $fgetc(fd);
    e1 = $ferror(fd, msg);
    r = $rewind(fd);
    e2 = $ferror(fd, msg);
    $fclose(fd);
    $display("g=%0d e1=%0d r=%0d e2=%0d msg=[%s]", g, e1 != 0, r, e2, msg);
    $finish;
  end
endmodule
