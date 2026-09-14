// Static-review regression; not executed during patch preparation.
module tb;
  int a, b, count, fd, c;
  real x, y;
  initial begin
    a = -1; b = -1;
    count = $sscanf("12,34", "%d,%d", a, b);
    if (count != 2 || a != 12 || b != 34) $fatal(1, "integer delimiter");
    count = $sscanf("-12:ab", "%d:%h", a, b);
    if (count != 2 || a != -12 || b != 'hab) $fatal(1, "radix delimiter");
    count = $sscanf("1.25,-2e+1", "%f,%f", x, y);
    if (count != 2 || x != 1.25 || y != -20.0) $fatal(1, "real prefix");
    count = $sscanf("1234", "%2d%2d", a, b);
    if (count != 2 || a != 12 || b != 34) $fatal(1, "field width");
    a = 91;
    count = $sscanf("bad,7", "%*d,%d", a);
    if (count != 0 || a != 91) $fatal(1, "suppressed conversion was not validated");
    count = $sscanf("12,7", "%*d,%d", a);
    if (count != 1 || a != 7) $fatal(1, "suppressed valid conversion");
    b = 92;
    count = $sscanf("12,bad", "%d,%d", a, b);
    if (count != 1 || a != 12 || b != 92) $fatal(1, "partial assignment");
    fd = $fopen("scan_prefix.txt", "w+");
    if (!fd) $fatal(1, "open");
    $fwrite(fd, "12,34");
    $rewind(fd);
    count = $fscanf(fd, "%d", a);
    if (count != 1 || a != 12 || $ftell(fd) != 2) $fatal(1, "file prefix/tell");
    c = $fgetc(fd);
    if (c != 44) $fatal(1, "delimiter was consumed");
    count = $fscanf(fd, "%d", b);
    if (count != 1 || b != 34) $fatal(1, "remaining file field");
    $fclose(fd);
    $display("numeric scanner ok");
    $finish(0);
  end
endmodule
