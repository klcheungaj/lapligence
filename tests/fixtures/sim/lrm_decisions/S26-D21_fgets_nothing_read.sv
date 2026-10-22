// Decision S26-D21: $fgets that reads nothing from an open file leaves an
// empty string destination.
//
// IEEE 1800-2009 21.3.4.2 (SystemVerilog-1800-2009.txt L36830-36831):
//   "reads characters from the file specified by fd into the variable str
//   until str is filled, or a newline character is read and transferred to
//   str, or an EOF condition is encountered."
// L36838: "If an error occurs reading from the file, then code is set to
//   zero."
//
// The text does not say what str holds when no character is read. llg
// follows industry-standard behaviour: at the end of a file, in an empty
// file and in a file open only for writing, $fgets returns 0 and a string
// destination becomes empty. A descriptor that is not open leaves it
// unchanged (S26-D2).
module tb;
  integer fd, c;
  string s;
  initial begin
    fd = $fopen("s26_d21.txt", "w");
    $fwrite(fd, "ab");
    $fclose(fd);
    fd = $fopen("s26_d21.txt", "r");
    c = $fgets(s, fd);
    $display("line c=%0d s=[%s]", c, s);
    s = "keep";
    c = $fgets(s, fd);
    $display("at end c=%0d s=[%s] len=%0d", c, s, s.len());
    $fclose(fd);
    fd = $fopen("s26_d21_empty.txt", "w");
    $fclose(fd);
    fd = $fopen("s26_d21_empty.txt", "r");
    s = "keep";
    c = $fgets(s, fd);
    $display("empty file c=%0d s=[%s]", c, s);
    $fclose(fd);
    fd = $fopen("s26_d21_out.txt", "w");
    s = "keep";
    c = $fgets(s, fd);
    $display("write-only c=%0d s=[%s]", c, s);
    $fclose(fd);
    s = "keep";
    c = $fgets(s, fd);
    $display("closed c=%0d s=[%s]", c, s);
    $finish;
  end
endmodule
