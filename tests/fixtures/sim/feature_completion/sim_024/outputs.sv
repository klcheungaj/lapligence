// SIM-024: file and string outputs share the display formatter; writing to
// a closed descriptor records an error for `$ferror` (SV 21.3).
module tb;
  int fd, n, err;
  string line, msg;
  int q [$];
  initial begin
    q = '{1, 2};
    fd = $fopen("sim024_out.txt", "w");
    $fdisplay(fd, "A|%d|%p|", 8'd7, q);
    $fwrite(fd, "B|%h|\n", 8'hab);
    $fclose(fd);
    fd = $fopen("sim024_out.txt", "r");
    n = $fgets(line, fd);
    $write("%s", line);
    n = $fgets(line, fd);
    $write("%s", line);
    $fclose(fd);
    $fdisplay(fd, "lost");
    err = $ferror(fd, msg);
    $display("C|%0d|%s|", err != 0, msg);
    $fdisplay(1, "D|%0d|%p|", 5, q);
    $fwrite(32'h8000_0001, "E|%s|\n", "out");
    $finish(0);
  end
endmodule
