// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_003/time_zero.v
// IEEE 1364-2001 8.1.3, 8.1.5, 8.5; IEEE 1800-2009 29.3.3, 29.3.5, 29.7.
// Decision S3-D4: when simulation begins every input of a sequential UDP is
// taken as x and the output holds its initial value (x without one). An
// input that is known at time zero (a constant or supply net) is a
// transition from x; an undriven (z) input is not a transition, so the
// initial value stays until a table row says otherwise.
primitive rise_set (q, c);
    output q; reg q;
    input c;
    initial q = 0;
    table
        p : ? : 1 ;
        n : ? : 0 ;
    endtable
endprimitive

primitive keep (q, c);
    output q; reg q;
    input c;
    initial q = 1'b1;
    table
        r : ? : 0 ;
        f : ? : - ;
    endtable
endprimitive

primitive plain (q, c);
    output q; reg q;
    input c;
    table
        (x1) : ? : 1 ;
        (x0) : ? : 0 ;
        (01) : ? : 1 ;
        (10) : ? : 0 ;
    endtable
endprimitive

module tb;
    supply1 vdd;
    supply0 gnd;
    wire floating;
    wire q_const1, q_vdd, q_gnd, q_float, q_keep_float, q_keep_const, q_plain;
    rise_set u_const1 (q_const1, 1'b1);
    rise_set u_vdd (q_vdd, vdd);
    rise_set u_gnd (q_gnd, gnd);
    rise_set u_float (q_float, floating);
    keep u_keep_float (q_keep_float, floating);
    keep u_keep_const (q_keep_const, 1'b1);
    plain u_plain (q_plain, floating);

    initial begin
        #1 $display("const1=%b vdd=%b gnd=%b float=%b keep_float=%b keep_const=%b plain=%b",
                    q_const1, q_vdd, q_gnd, q_float, q_keep_float, q_keep_const, q_plain);
        $finish(0);
    end
endmodule
