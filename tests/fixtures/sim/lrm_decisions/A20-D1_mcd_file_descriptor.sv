// Decision A20-D1: a descriptor with bit 31 set is a $fopen file descriptor,
// not a multichannel descriptor. vpi_mcd_printf returns EOF and writes
// nothing to it, vpi_mcd_flush returns nonzero, and vpi_mcd_close returns it
// unclosed, each with a vpiError status; the file stays open for the design.
// vpi_mcd_open of a file that is open only as a file descriptor returns 0
// with vpiError: it neither returns the descriptor nor opens a second stream
// that would truncate the file.
//
// IEEE 1800-2009 38.27 (SystemVerilog-1800-2009.txt L63633-63636):
//   "The mcd descriptors returned from vpi_mcd_open() and from $fopen may be
//   shared between the built-in system tasks that use mcd descriptors and the
//   VPI routines that use mcd descriptors. If the most significant bit of the
//   return value from $fopen is set, then the value is an fd file descriptor,
//   which is not compatible with the mcd descriptor returned by
//   vpi_mcd_open()."
// 38.28 (L63678-63679): "vpi_mcd_printf() shall not write to a file
//   represented by an fd file descriptor returned from $fopen".
// 38.24 (L63515) says vpi_mcd_close returns "the mcd value of the unclosed
// channels" on error; it is silent on a file descriptor, as 38.25 is. 38.27
// (L63639-63641) has vpi_mcd_open "return the descriptor number" of a file
// "already opened ... using $fopen", which for a file descriptor would be
// the incompatible fd. Refusing a file descriptor everywhere but
// vpi_mcd_name is an llg choice.
//
// Build A20-D1_mcd_file_descriptor.c as a VPI application (llg: static
// archive given to --dpi-lib; `task $d1(input integer fd)` declared as a
// system task).
module tb;
    integer fd, code;
    string line;
    initial begin
        fd = $fopen("a20_d1.txt", "w");
        $d1(fd);
        $fdisplay(fd, "design line");
        $fclose(fd);
        fd = $fopen("a20_d1.txt", "r");
        while (!$feof(fd)) begin
            code = $fgets(line, fd);
            if (code > 0) $write("file: %s", line);
        end
        $fclose(fd);
        $finish;
    end
endmodule
