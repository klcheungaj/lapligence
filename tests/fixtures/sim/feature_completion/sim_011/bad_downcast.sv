// SIM-011 A03: an invalid downcast (SV 8.16): the function form of `$cast`
// returns 0 and leaves the destination unchanged; the task form is a
// run-time error at the call.
class A;
    int a = 1;
endclass

class B extends A;
    int b = 2;
endclass

module tb;
    A base;
    B derived;
    B kept;

    initial begin
        kept = new;
        derived = kept;
        base = new;
        $display("fn=%0d same=%0d", $cast(derived, base), derived == kept);
        base = kept;
        $display("ok=%0d b=%0d", $cast(derived, base), derived.b);
        base = new;
        $cast(derived, base);
        $display("after same=%0d", derived == kept);
        $finish;
    end
endmodule
