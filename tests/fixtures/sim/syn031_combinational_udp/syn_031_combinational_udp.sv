// llg-test-fixture: tests/fixtures/sim/syn031_combinational_udp/syn_031_combinational_udp.sv
// IEEE 1364-2001 8.1.6, 8.2, 8.6; IEEE 1800-2009 29.3.5, 29.4, 29.8.
primitive udp_mux(y, sel, a, b);
    output y;
    input sel, a, b;
    table
        0 0 ? : 0;
        0 1 ? : 1;
        1 ? 0 : 0;
        1 ? 1 : 1;
        x 0 0 : 0;
        x 1 1 : 1;
    endtable
endprimitive

primitive udp_parity(y, a, b);
    output y;
    input a, b;
    table
        0 0 : 0;
        0 1 : 1;
        1 0 : 1;
        1 1 : 0;
        x ? : x;
        b x : x;
    endtable
endprimitive

module tb;
    reg sel, a, b, p, q, ext;
    reg [1:0] va, vb;
    wire mux_y, mux_ref, alt_y, alt_ref, parity_y, parity_ref;
    wire [1:0] array_y, array_ref;
    wire resolved_y, resolved_ref, delayed_y, delayed_ref;
    wire norm_a, norm_b;

    assign norm_a = a | a;
    assign norm_b = b | b;
    assign mux_ref = sel ? norm_b : norm_a;
    assign alt_ref = sel ? norm_a : norm_b;
    assign parity_ref = p ^ q;
    udp_mux main_mux(mux_y, sel, a, b);
    udp_mux alt_mux(alt_y, sel, b, a);
    udp_parity parity(parity_y, p, q);
    udp_parity bank[1:0](array_y, va, vb);
    xor gate_bank[1:0](array_ref, va, vb);

    udp_mux on_net(resolved_y, sel, a, b);
    buf independent_driver(resolved_y, ext);
    assign resolved_ref = mux_ref;
    buf independent_reference(resolved_ref, ext);
    udp_mux #2 delayed(delayed_y, sel, a, b);
    buf #2 delayed_reference(delayed_ref, mux_ref);

    initial begin
        sel = 0; a = 0; b = 1; p = 0; q = 0;
        va = 2'b00; vb = 2'b11; ext = 0;
        #1 $display("start mux=%b/%b alt=%b/%b parity=%b/%b array=%b/%b resolved=%b/%b delay=%b/%b",
                    mux_y, mux_ref, alt_y, alt_ref, parity_y, parity_ref,
                    array_y, array_ref, resolved_y, resolved_ref, delayed_y, delayed_ref);
        #1 $display("delay0 %b/%b", delayed_y, delayed_ref);

        sel = 1; a = 0; b = 1; p = 0; q = 1;
        va = 2'b01; vb = 2'b11; ext = 1;
        #1 $display("changed mux=%b/%b alt=%b/%b parity=%b/%b array=%b/%b resolved=%b/%b delay=%b/%b",
                    mux_y, mux_ref, alt_y, alt_ref, parity_y, parity_ref,
                    array_y, array_ref, resolved_y, resolved_ref, delayed_y, delayed_ref);
        #1 $display("delay1 %b/%b", delayed_y, delayed_ref);

        sel = 1'bx; a = 0; b = 0; p = 1'bx; q = 0;
        va = 2'bz1; vb = 2'b01; ext = 1;
        #2 $display("same_known mux=%b/%b parity=%b/%b array=%b/%b resolved=%b/%b delay=%b/%b",
                    mux_y, mux_ref, parity_y, parity_ref, array_y, array_ref,
                    resolved_y, resolved_ref, delayed_y, delayed_ref);
        sel = 1'bz; a = 1; b = 1; p = 1'bz; q = 1;
        #2 $display("z_control mux=%b/%b parity=%b/%b resolved=%b/%b", mux_y, mux_ref,
                    parity_y, parity_ref, resolved_y, resolved_ref);
        sel = 1'bx; a = 0; b = 1; p = 0; q = 1'bx; ext = 0;
        #2 $display("unmatched mux=%b/%b alt=%b/%b parity=%b/%b resolved=%b/%b",
                    mux_y, mux_ref, alt_y, alt_ref, parity_y, parity_ref, resolved_y, resolved_ref);
        sel = 0; a = 1'bz; b = 1'bx; p = 1; q = 1'bz;
        #2 $display("z_input mux=%b/%b parity=%b/%b resolved=%b/%b", mux_y, mux_ref,
                    parity_y, parity_ref, resolved_y, resolved_ref);
        sel = 1; a = 1'bz; b = 0; p = 1; q = 1;
        #2 $display("wildcard mux=%b/%b parity=%b/%b resolved=%b/%b", mux_y, mux_ref,
                    parity_y, parity_ref, resolved_y, resolved_ref);
        $finish(0);
    end
endmodule
