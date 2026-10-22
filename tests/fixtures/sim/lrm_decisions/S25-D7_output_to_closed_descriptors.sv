// Decision S25-D7: output to descriptors that are not open. A write through
// a closed file descriptor, or through a multichannel descriptor whose
// channel is closed, writes nothing there and the run continues (the
// simulator may warn); the other channels of a multichannel descriptor still
// receive the output. $ferror of a multichannel descriptor or of a closed
// file descriptor returns 0 and leaves its string argument unchanged.
//
// IEEE 1800-2009 21.3.1 (SystemVerilog-1800-2009.txt L36633-36634): "The
// $fclose system task closes the file specified by fd or closes the file(s)
// specified by the multichannel descriptor mcd. No further output to or
// input from any file descriptor(s) closed by $fclose is allowed." 21.3.7
// L37203-37205: "The integral value of the error code is returned in errno.
// If the most recent operation did not result in an error, then the value
// returned shall be zero, and the str variable shall be cleared." The text
// does not say what an output task or $ferror does with a descriptor that is
// not open; llg follows industry-standard behaviour.
module tb;
  integer fd, mcd, err;
  string msg;
  reg [3:0] a = 1;
  initial begin
    fd = $fopen("s25_d7.txt", "w");
    $fclose(fd);
    mcd = $fopen("s25_d7_mcd.txt");
    $fclose(mcd);
    $fdisplay(fd, "closed fd");
    $fwrite(fd, "closed fd");
    $fdisplay(mcd | 1, "stdout of a partly closed multichannel descriptor");
    $fmonitor(fd, "closed fd monitor %0d", a);
    msg = "kept";
    err = $ferror(fd, msg);
    $display("closed fd: err=%0d msg=%s", err, msg);
    err = $ferror(mcd, msg);
    $display("multichannel: err=%0d msg=%s", err, msg);
    err = $ferror(1, msg);
    $display("stdout channel: err=%0d msg=%s", err, msg);
    #1 a = 2;
    #1 $display("done");
    $finish;
  end
endmodule
