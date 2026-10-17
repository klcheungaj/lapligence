// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_003/same_slot.v
// IEEE 1364-2001 8.4, 5.4-5.6; IEEE 1800-2009 29.6, 4.4-4.7. Repeated input
// changes in one time slot: a UDP evaluates each change it sees, so changes
// separated by #0 (the Inactive region runs only after the Active region,
// where the UDP evaluation event runs) are separate transitions. Changes of
// several inputs that one evaluation sees are processed one transition at a
// time in port order (decision S3-D2): with the clock first, a rising clock
// still samples the old data.
primitive toggle_any (q, t);
    output q; reg q;
    input t;
    initial q = 0;
    table
        * : 0 : 1 ;
        * : 1 : 0 ;
    endtable
endprimitive

primitive dff (q, clk, d);
    output q; reg q;
    input clk, d;
    initial q = 0;
    table
        r 0 : ? : 0 ;
        r 1 : ? : 1 ;
        (?0) ? : ? : - ;
        ? * : ? : - ;
    endtable
endprimitive

module tb;
    reg t, clk, d;
    wire q, qd;
    toggle_any u (q, t);
    dff v (qd, clk, d);

    initial begin
        t = 1'b0;
        clk = 1'b0;
        d = 1'b0;
        #1 $display("start q=%b qd=%b", q, qd);
        t = 1'b1;
        #0 t = 1'b0;
        #0 t = 1'b1;
        #1 $display("three q=%b", q);
        t = 1'b0;
        #0 t = 1'b1;
        #1 $display("two q=%b", q);
        {clk, d} = 2'b11;
        #1 $display("clock_first qd=%b", qd);
        clk = 1'b0;
        #1 clk = 1'b1;
        #1 $display("next_edge qd=%b", qd);
        $finish(0);
    end
endmodule
