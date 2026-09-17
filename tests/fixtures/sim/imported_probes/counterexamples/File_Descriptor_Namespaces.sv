// Static-review counterexample; NOT EXECUTED.
module tb;
  integer fd, mcd;
  initial begin
    fd = $fopen("review_fd.tmp", "w");
    mcd = $fopen("review_mcd.tmp");
    if (fd == 0 || mcd == 0) $fatal(1, "counterexample needs a writable current directory");
    if (fd[31] !== 1'b1 || mcd[31] !== 1'b0) $fatal(1, "FD/MCD tag mismatch");
    $fwrite(32'h8000_0001, "standard output FD\n");
    $fclose(fd); $fclose(mcd);
    $finish(0);
  end
endmodule
