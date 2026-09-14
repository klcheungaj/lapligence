// Static-review regression; not executed during patch preparation.
`timescale 1ns/1ps
module tb;
  int fd, mcd, status, reader, value;
  string line;
  initial begin
    fd = $fopen("descriptor_fd.txt", "wb+");
    mcd = $fopen("descriptor_mcd.txt");
    if (!fd || !mcd || !fd[31] || mcd[31] || (mcd & (mcd-1)) != 0)
      $fatal(1, "descriptor namespaces");
    // FD stored in a signed int must still be accepted.
    $fdisplay(fd, "fd data");
    $fdisplay(mcd | 1, "mcd fanout");
    $fwrite(32'h8000_0001, "standard stdout\n");
    $fflush(fd);
    $rewind(fd);
    status = $fgets(line, fd);
    if (status != 8 || line != "fd data\n") $fatal(1, "signed FD access");
    // Closing a descriptor cancels pending deferred output before its reuse.
    $fstrobe(fd, "BAD");
    $fmonitor(fd, "BAD monitor %d", value);
    $fclose(fd);
    fd = $fopen("descriptor_reused.txt", "w+");
    #1;
    $fwrite(fd, "new");
    $rewind(fd);
    status = $fgets(line, fd);
    if (status != 3 || line != "new") $fatal(1, "deferred output survived close");
    $fclose(fd);
    $fclose(mcd);
    reader = $fopen("descriptor_fd.txt", "rb+");
    if (!reader || !reader[31]) $fatal(1, "rb+ mode");
    $fclose(reader);
    reader = $fopen("descriptor_fd.txt", "ab+");
    if (!reader || !reader[31]) $fatal(1, "ab+ mode");
    $fclose(reader);
    $display("descriptor namespaces ok");
    $finish(0);
  end
endmodule
