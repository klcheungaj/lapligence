// Decision A20-D3: vpi_mcd_printf to several channels writes the text to each
// and returns its character count once (not multiplied by the channels).
// Channels of the descriptor that are not open are skipped: when at least one
// channel is open the text reaches it and the count is returned with a
// vpiError status; when none is open, EOF is returned with vpiError.
//
// IEEE 1800-2009 38.28 (SystemVerilog-1800-2009.txt L63682-63687):
//   "Several channels can be written to simultaneously because channels are
//   represented by discrete bits in the integer mcd. ... The routine shall
//   return the number of characters printed or return EOF if an error
//   occurred."
// The text is silent on a count for several channels and on a descriptor
// with closed channels; skipping closed channels follows $fdisplay
// (decision S25-D7). Both are llg choices.
//
// Build A20-D3_mcd_printf_channels.c as a VPI application (llg: static
// archive given to --dpi-lib; `task $d3(input integer a, input integer b)`
// declared as a system task).
module tb;
    integer a, b, code;
    string line;
    initial begin
        a = $fopen("a20_d3_a.txt");
        b = $fopen("a20_d3_b.txt");
        $d3(a, b);
        $fclose(a);
        $fclose(b);
        a = $fopen("a20_d3_a.txt", "r");
        while (!$feof(a)) begin
            code = $fgets(line, a);
            if (code > 0) $write("a: %s", line);
        end
        $fclose(a);
        b = $fopen("a20_d3_b.txt", "r");
        while (!$feof(b)) begin
            code = $fgets(line, b);
            if (code > 0) $write("b: %s", line);
        end
        $fclose(b);
        $finish;
    end
endmodule
