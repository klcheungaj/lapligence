// Decision S3-D4: when simulation begins, every input of a sequential UDP is
// taken as x and the output holds its initial value (x without an initial
// statement). An input that is known at time zero, such as a constant
// terminal, is a transition from x at time zero; an undriven (z) input makes
// no transition, so the initial value stays. The initial value is driven at
// time zero without the instance delay.
//
// IEEE 1800-2009 29.3.3 (SystemVerilog-1800-2009.txt L48761): "The
// sequential UDP initial statement specifies the value of the output port
// when simulation begins." 29.7 L49064-49066: "a delay specification on an
// instantiated UDP does not delay the simulation time of the assignment of
// this initial value to the output. When simulation starts, this value is the
// current state in the state table." The value of the inputs before their
// first change is not stated.
primitive rise_set (q, c);
    output q;
    reg q;
    input c;
    initial q = 0;
    table
        p : ? : 1 ;
        n : ? : 0 ;
    endtable
endprimitive

primitive keep (q, c);
    output q;
    reg q;
    input c;
    initial q = 1'b1;
    table
        r : ? : 0 ;
        f : ? : - ;
    endtable
endprimitive

module tb;
    wire floating;
    wire a, b, c, d;
    rise_set u_const (a, 1'b1);
    rise_set u_float (b, floating);
    keep #5 u_delayed (c, floating);
    keep u_unspecified (d, 1'b1);
    initial begin
        #1 $display("const=%b float=%b delayed=%b unspecified=%b", a, b, c, d);
        $finish;
    end
endmodule
