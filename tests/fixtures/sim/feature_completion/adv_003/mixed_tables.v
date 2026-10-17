// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_003/mixed_tables.v
// IEEE 1364-2001 8.7-8.8; IEEE 1800-2009 29.9-29.10. The jk_edge_ff example
// mixes level-sensitive preset/clear rows with clock and j/k edge rows. In
// `dom` a level row and an edge row both match a rising clock with data 0
// and give different next states: the level row decides. Each step changes
// one input.
primitive jk_edge_ff (q, clock, j, k, preset, clear);
    output q; reg q;
    input clock, j, k, preset, clear;
    table
    // clock jk pc state output/next state
        ?   ?? 01 : ? : 1 ; // preset logic
        ?   ?? *1 : 1 : 1 ;
        ?   ?? 10 : ? : 0 ; // clear logic
        ?   ?? 1* : 0 : 0 ;
        r   00 00 : 0 : 1 ; // normal clocking cases
        r   00 11 : ? : - ;
        r   01 11 : ? : 0 ;
        r   10 11 : ? : 1 ;
        r   11 11 : 0 : 1 ;
        r   11 11 : 1 : 0 ;
        f   ?? ?? : ? : - ;
        b   *? ?? : ? : - ; // j and k transition cases
        b   ?* ?? : ? : - ;
    endtable
endprimitive

primitive dom (q, c, d);
    output q; reg q;
    input c, d;
    table
    // c d : q : q+
        r 0 : ? : 0 ;  // edge row
        1 0 : ? : 1 ;  // level row for the same inputs: dominates
        r 1 : ? : 1 ;
        f ? : ? : - ;
        0 * : ? : - ;
        1 (01) : ? : - ;
        1 (10) : ? : - ;
    endtable
endprimitive

module tb;
    reg clock, j, k, preset, clear;
    reg c, d;
    wire q, qd;
    jk_edge_ff u (q, clock, j, k, preset, clear);
    dom v (qd, c, d);

    task show(input [8*12-1:0] label);
        #1 $display("%0s clk=%b jk=%b%b pc=%b%b q=%b", label, clock, j, k, preset, clear, q);
    endtask

    task show_dom(input [8*12-1:0] label);
        #1 $display("%0s c=%b d=%b qd=%b", label, c, d, qd);
    endtask

    initial begin
        show("start");
        clock = 1'b0;  show("clk0");
        clear = 1'b1;  show("clear1");
        preset = 1'b0; show("preset");
        preset = 1'b1; show("preset_rel");
        j = 1'b0;      show("j0");
        k = 1'b0;      show("k0");
        clock = 1'b1;  show("r_00");
        clock = 1'b0;  show("f");
        k = 1'b1;      show("k1");
        clock = 1'b1;  show("r_01");
        clock = 1'b0;  show("f2");
        j = 1'b1;      show("j1");
        clock = 1'b1;  show("r_11_s0");
        clock = 1'b0;  show("f3");
        clear = 1'b0;  show("clear");
        clear = 1'b1;  show("clear_rel");
        clock = 1'b1;  show("r_11_s0b");
        clock = 1'b0;  show("f4");
        clock = 1'b1;  show("r_11_s1");

        show_dom("dom_start");
        c = 1'b0;      show_dom("c0");
        d = 1'b0;      show_dom("d0");
        c = 1'b1;      show_dom("rise_d0");
        c = 1'b0;      show_dom("fall");
        d = 1'b1;      show_dom("d1");
        c = 1'b1;      show_dom("rise_d1");
        d = 1'b0;      show_dom("level_d0");
        $finish(0);
    end
endmodule
