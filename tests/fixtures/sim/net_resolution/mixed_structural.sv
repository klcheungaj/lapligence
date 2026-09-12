// llg-test-fixture: tests/fixtures/sim/net_resolution/mixed_structural.sv
module source(
    input wire i,
    output wire out_wire,
    output wand out_wand,
    output wor out_wor
);
    assign out_wire = i;
    assign out_wand = i;
    assign out_wor = i;
endmodule

module selected_source(input wire en, output wire [7:0] out);
    assign out[3:0] = en ? 4'hf : 4'hz;
endmodule

module tb;
    reg drive, gate_drive, port_drive, en;
    wire w;
    wand wa;
    wor wo;
    tri0 t0;
    tri1 t1;
    supply0 s0;
    supply1 s1;
    wire [7:0] selected;

    assign w = drive;
    buf gw(w, gate_drive);
    assign wa = drive;
    buf ga(wa, gate_drive);
    assign wo = drive;
    buf go(wo, gate_drive);
    source u(.i(port_drive), .out_wire(w), .out_wand(wa), .out_wor(wo));
    assign t0 = 1'bz;
    assign t1 = 1'bz;
    assign s0 = 1'bz;
    assign s1 = 1'bz;
    assign selected[7:4] = 4'h0;
    selected_source us(.en(en), .out(selected));

    initial begin
        drive = 1'b1;
        gate_drive = 1'b0;
        port_drive = 1'b1;
        en = 1'b1;
        #1 $display("mixed=%b%b%b/%b%b%b%b", w, wa, wo, t0, t1, s0, s1);
        $display("selected=%h", selected);
        drive = 1'b0;
        gate_drive = 1'b1;
        port_drive = 1'b0;
        en = 1'b0;
        #1 $display("mixed=%b%b%b/%b%b%b%b", w, wa, wo, t0, t1, s0, s1);
        $display("selected=%h", selected);
        drive = 1'bz;
        gate_drive = 1'bz;
        port_drive = 1'bz;
        #1 $display("mixed=%b%b%b/%b%b%b%b", w, wa, wo, t0, t1, s0, s1);
        $finish(0);
    end
endmodule
