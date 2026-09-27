// llg-test-fixture: tests/fixtures/sim/waveform/output_redirect.sv
// LRM: IEEE 1364-2001 17.2.1, 17.2.8, 17.2.9 and 18.1.1.
`timescale 1ns/1ps
module tb;
    reg [7:0] mem [0:1];
    reg [7:0] value;
    integer fd;

    initial begin
        $readmemh("input.hex", mem);
        $dumpfile("trace.vcd");
        $dumpvars(0, tb);
        value = mem[0];
        fd = $fopen("note.txt", "w");
        $fdisplay(fd, "note %h", value);
        $fclose(fd);
        mem[1] = value + 8'h01;
        $writememh("mem.hex", mem);
        $display("console %h", value);
        #1 value = 8'h5a;
        #1 $finish(0);
    end
endmodule
