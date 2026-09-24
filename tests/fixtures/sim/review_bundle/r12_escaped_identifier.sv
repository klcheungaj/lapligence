// llg-test-fixture: tests/fixtures/sim/review_bundle/r12_escaped_identifier.sv
// IEEE 1364-2001 §2.7 / IEEE 1800-2009 §5.6: escaped identifiers terminate
// at whitespace and keep punctuation names distinct from ordinary identifiers.
module tb;
    reg \a.b ;
    reg a_b;
    reg \a-b ;

    initial begin
        \a.b  = 1'b1;
        a_b = 1'b0;
        \a-b  = 1'b1;
        $display("dot=%0b under=%0b dash=%0b", \a.b , a_b, \a-b );
        $finish(0);
    end
endmodule
