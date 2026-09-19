// llg-test-fixture: tests/fixtures/sim/expression_mutations/postincrement_result.sv
// IEEE 1800-2009 11.4.2: x++ yields the old value, ++x yields the new value.
module tb;
    integer a;
    integer old;
    integer after;

    initial begin
        a = 5;
        old = a++;
        after = a;
        $display("post_inc old=%0d a=%0d after=%0d", old, a, after);

        a = 5;
        old = ++a;
        after = a;
        $display("pre_inc old=%0d a=%0d after=%0d", old, a, after);

        a = 5;
        old = a--;
        after = a;
        $display("post_dec old=%0d a=%0d after=%0d", old, a, after);

        a = 5;
        old = --a;
        after = a;
        $display("pre_dec old=%0d a=%0d after=%0d", old, a, after);

        $finish(0);
    end
endmodule
