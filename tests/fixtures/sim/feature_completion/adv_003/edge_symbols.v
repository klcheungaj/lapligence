// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_003/edge_symbols.v
// IEEE 1364-2001 8.1.4-8.1.6, 8.4 (Table 40); IEEE 1800-2009 29.3.4-29.3.6,
// 29.6 (Table 29-1). One detector per edge symbol family on the same input:
// r = (01), f = (10), p = (01)(0x)(x1), n = (10)(1x)(x0), * = (??), explicit
// (vw) pairs with 0 1 x ? b, and the X default of an unspecified transition.
// The input visits every transition between 0, 1, x and z; z is read as x,
// so z -> x and x -> z are no transitions.
primitive det_r (q, c);
    output q; reg q; input c;
    table
        r    : ? : 1 ;
        (0x) : ? : 0 ;
        (10) : ? : 0 ;
        (1x) : ? : 0 ;
        (x0) : ? : 0 ;
        (x1) : ? : 0 ;
    endtable
endprimitive

primitive det_f (q, c);
    output q; reg q; input c;
    table
        f    : ? : 1 ;
        (01) : ? : 0 ;
        (0x) : ? : 0 ;
        (1x) : ? : 0 ;
        (x0) : ? : 0 ;
        (x1) : ? : 0 ;
    endtable
endprimitive

primitive det_p (q, c);
    output q; reg q; input c;
    table
        p : ? : 1 ;
        n : ? : 0 ;
    endtable
endprimitive

primitive det_n (q, c);
    output q; reg q; input c;
    table
        n : ? : 1 ;
        p : ? : 0 ;
    endtable
endprimitive

// Toggles on every value change: each change is one transition.
primitive det_star (q, c);
    output q; reg q; input c;
    table
        * : 0 : 1 ;
        * : 1 : 0 ;
        * : x : 0 ;
    endtable
endprimitive

// `?` and `b` inside explicit transitions; (?x) is written as (bx).
primitive det_q (q, c);
    output q; reg q; input c;
    table
        (?1) : ? : 1 ;
        (?0) : ? : 0 ;
        (bx) : ? : x ;
    endtable
endprimitive

// Only rising edges are specified: every other transition gives X.
primitive det_unspec (q, c);
    output q; reg q; input c;
    table
        r : ? : 1 ;
    endtable
endprimitive

module tb;
    reg c;
    wire qr, qf, qp, qn, qs, qq, qu;
    det_r u_r (qr, c);
    det_f u_f (qf, c);
    det_p u_p (qp, c);
    det_n u_n (qn, c);
    det_star u_s (qs, c);
    det_q u_q (qq, c);
    det_unspec u_u (qu, c);

    task step(input v);
        begin
            c = v;
            #1 $display("c=%b r=%b f=%b p=%b n=%b star=%b q=%b unspec=%b",
                        c, qr, qf, qp, qn, qs, qq, qu);
        end
    endtask

    initial begin
        #1 $display("start r=%b f=%b p=%b n=%b star=%b q=%b unspec=%b",
                    qr, qf, qp, qn, qs, qq, qu);
        step(1'b0);
        step(1'b1);
        step(1'b0);
        step(1'bx);
        step(1'b0);
        step(1'b1);
        step(1'bx);
        step(1'b1);
        step(1'bz);
        step(1'bx);
        step(1'b0);
        step(1'bz);
        step(1'b1);
        step(1'b1);
        step(1'bx);
        step(1'bz);
        step(1'b0);
        $finish(0);
    end
endmodule
