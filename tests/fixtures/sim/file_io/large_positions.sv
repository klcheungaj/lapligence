// $fseek and $ftell positions beyond 2 GiB on a 4-byte file, reached by
// relative seeks with 32-bit offsets: the positions need 64 bits, and the
// file keeps its 4 bytes.
module tb;
  integer fd;
  integer status;
  initial begin
    fd = $fopen("large_positions.bin", "wb");
    $fwrite(fd, "abcd");
    status = $fseek(fd, 2000000000, 0);
    status = $fseek(fd, 2000000000, 1);
    $display("current=%0d tell=%0d", status, $ftell(fd));
    status = $fseek(fd, -2000000000, 1);
    status = $fseek(fd, -1999999998, 1);
    $display("back=%0d tell=%0d", status, $ftell(fd));
    status = $fseek(fd, 2000000000, 2);
    status = $fseek(fd, 2000000000, 1);
    $display("end=%0d tell=%0d", status, $ftell(fd));
    status = $fseek(fd, -1, 0);
    $display("before_start=%0d", status);
    $fclose(fd);
    $finish;
  end
endmodule
