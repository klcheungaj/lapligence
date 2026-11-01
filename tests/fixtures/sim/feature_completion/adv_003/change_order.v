// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_003/change_order.v
// IEEE 1800-2009 4.3, 4.7, 29.5, 29.6. Several inputs that change before
// the UDP is evaluated are taken one transition at a time in the order they
// changed (decision S3-D5): the writer's statement order, nonblocking update
// order and the order in which separate processes wrote them, each change
// seeing the new values of the inputs changed before it and the old values
// of the others. The cases cover reg, net and bit-select inputs, a module
// whose instances share one body, a ten-input row-scan definition and a
// zero-width clock pulse, which is not seen.
primitive latch (q, d, en);
    output q; reg q;
    input d, en;
    table
    // d en : q : q+
       0  1 : ? : 0 ;
       1  1 : ? : 1 ;
       ?  0 : ? : - ;
    endtable
endprimitive

// Ten inputs with an edge on every one: above the dense next-state limit.
primitive wide (q, c, d, e, f, g, h, i, j, k, l);
    output q; reg q;
    input c, d, e, f, g, h, i, j, k, l;
    initial q = 0;
    table
    // c d e f g h i j k l : q : q+
       r 0 ? ? ? ? ? ? ? ? : ? : 0 ;
       r 1 ? ? ? ? ? ? ? ? : ? : 1 ;
       n ? ? ? ? ? ? ? ? ? : ? : - ;
       (0x) ? ? ? ? ? ? ? ? ? : ? : - ;
       (x1) ? ? ? ? ? ? ? ? ? : ? : - ;
       ? * ? ? ? ? ? ? ? ? : ? : - ;
       ? ? * ? ? ? ? ? ? ? : ? : - ;
       ? ? ? * ? ? ? ? ? ? : ? : - ;
       ? ? ? ? * ? ? ? ? ? : ? : - ;
       ? ? ? ? ? * ? ? ? ? : ? : - ;
       ? ? ? ? ? ? * ? ? ? : ? : - ;
       ? ? ? ? ? ? ? * ? ? : ? : - ;
       ? ? ? ? ? ? ? ? * ? : ? : - ;
       ? ? ? ? ? ? ? ? ? * : ? : - ;
    endtable
endprimitive

module hold (q, d, en);
    output q;
    input d, en;
    latch l (q, d, en);
endmodule

module tb;
    reg d1, e1, d2, e2;
    wire nd1, ne1, nd2, ne2, n1, n2;
    assign nd1 = d1;
    assign ne1 = e1;
    assign nd2 = d2;
    assign ne2 = e2;
    latch ln1 (n1, nd1, ne1);
    latch ln2 (n2, nd2, ne2);
    reg [3:0] v1, v2;
    wire b1, b2;
    latch lb1 (b1, v1[2], v1[0]);
    latch lb2 (b2, v2[2], v2[0]);
    reg [1:0] p;
    wire bp;
    latch lp (bp, p[1], p[0]);
    reg sd1, se1, sd2, se2;
    wire s1, s2;
    hold h1 (s1, sd1, se1);
    hold h2 (s2, sd2, se2);
    reg pd3, pe3, pd4, pe4;
    wire q3, q4;
    latch l3 (q3, pd3, pe3);
    latch l4 (q4, pd4, pe4);
    reg wc1, wd1, wc2, wd2, pc, pd;
    wire w1, w2, pq;
    wide u1 (w1, wc1, wd1, 1'b0, 1'b0, 1'b0, 1'b0, 1'b0, 1'b0, 1'b0, 1'b0);
    wide u2 (w2, wc2, wd2, 1'b0, 1'b0, 1'b0, 1'b0, 1'b0, 1'b0, 1'b0, 1'b0);
    wide u3 (pq, pc, pd, 1'b0, 1'b0, 1'b0, 1'b0, 1'b0, 1'b0, 1'b0, 1'b0);

    initial begin
        {d1, e1, d2, e2, sd1, se1, sd2, se2, pd3, pe3, pd4, pe4} = 12'hfff;
        v1 = 4'b0101;
        v2 = 4'b0101;
        p = 2'b11;
        {wc1, wd1, wc2, wd2} = 4'h0;
        pc = 1'b0;
        pd = 1'b1;
        #1 pc = 1'b1;
        #1 $display("open n1=%b n2=%b b1=%b b2=%b bp=%b s1=%b s2=%b pq=%b", n1, n2, b1, b2, bp, s1, s2, pq);
        e1 = 1'b0;
        d1 = 1'b0;
        d2 = 1'b0;
        e2 = 1'b0;
        v1[0] = 1'b0;
        v1[2] = 1'b0;
        v2[2] = 1'b0;
        v2[0] = 1'b0;
        p = 2'b00;
        se1 = 1'b0;
        sd1 = 1'b0;
        sd2 = 1'b0;
        se2 = 1'b0;
        wc1 = 1'b1;
        wd1 = 1'b1;
        wd2 = 1'b1;
        wc2 = 1'b1;
        pc = 1'b0;
        pd = 1'b0;
        pc = 1'b1;
        #1 $display("nets n1=%b n2=%b", n1, n2);
        $display("bits b1=%b b2=%b one_write=%b", b1, b2, bp);
        $display("shared s1=%b s2=%b", s1, s2);
        $display("wide clock_first=%b data_first=%b pulse=%b", w1, w2, pq);
        $display("processes q3=%b q4=%b", q3, q4);
        $finish(0);
    end
    initial #2 pe3 = 1'b0;
    initial #2 pd3 = 1'b0;
    initial #2 pd4 = 1'b0;
    initial #2 pe4 = 1'b0;
endmodule
