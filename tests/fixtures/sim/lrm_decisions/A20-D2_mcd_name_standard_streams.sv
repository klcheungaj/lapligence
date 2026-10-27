// Decision A20-D2: vpi_mcd_name names the standard streams: channel 1 (mcd 1)
// and the preopened file descriptor 32'h8000_0001 are "stdout",
// 32'h8000_0002 is "stderr" and 32'h8000_0000 is "stdin". A $fopen file has
// the name it was opened with.
//
// IEEE 1800-2009 38.26 (SystemVerilog-1800-2009.txt L63584-63589):
//   "The VPI routine vpi_mcd_name() shall return the name of a file
//   represented by a single-channel descriptor, cd. On error, the routine
//   shall return NULL. ... The channel descriptor cd could be an fd file
//   descriptor returned from $fopen (indicated by the most significant bit
//   being set) or an mcd multichannel descriptor".
// 21.3.1 (L36607): "The least significant bit (bit 0) of an mcd always refers
//   to the standard output." 21.3.1 also preopens STDIN, STDOUT and STDERR as
//   file descriptors 32'h8000_0000-32'h8000_0002 (L36617-36618). The text
//   gives these streams no file name; the names are an llg choice.
//
// Build A20-D2_mcd_name_standard_streams.c as a VPI application (llg: static
// archive given to --dpi-lib; `task $d2(input integer fd)` declared as a
// system task).
module tb;
    integer fd;
    initial begin
        fd = $fopen("a20_d2.txt", "w");
        $d2(fd);
        $fclose(fd);
        $finish;
    end
endmodule
