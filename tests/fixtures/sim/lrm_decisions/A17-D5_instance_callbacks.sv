// Decision A17-D5: callbacks on sequence and property instances. An
// instance that is the whole property of an assertion takes
// cbAssertionStart, cbAssertionSuccess and cbAssertionFailure; they run
// with that assertion's attempts, and a vacuous success of the attempt is a
// success of the instance. Any other reason fails, and so does a callback
// on an instance nested in another instance's arguments (it would need a
// hook in every sequence evaluation). The sequence instance of a cover
// property fails when an attempt does not match; under a cover sequence,
// which reports matches only, it gets no failure. vpi_control refuses an
// instance handle.
//
// IEEE 1800-2009 39.4.2 (SystemVerilog-1800-2009.txt L65667-L65669): "For
// property and sequence instances the start is the start of evaluation of
// the property or sequence instance."; L65670: "cbAssertionSuccess. An
// assertion attempt or property instance reaches a success state. For
// sequence instances, success is a match."; L65675-L65676:
// "cbAssertionFailure. An assertion attempt or a property fails to reach a
// success state or a sequence instance fails to match."; L65702-L65704:
// "The cbAssertionStart, cbAssertionSuccess, and cbAssertionFailure
// callbacks may also be registered on a sequence instance or a property
// instance."; 39.5.2 L65886-L65887: "Only assertion statement handles are
// valid here, not sequence or property instances." A vacuous success
// counting as a success and the refusal of nested instances are llg
// choices.
//
// Build A17-D5_instance_callbacks.c as a VPI application (llg: static
// archive given to --dpi-lib; `task $d5()` declared as a system task).
`timescale 1ns/1ns
module tb;
    logic clk = 0, a = 1, b = 1;
    always #5 clk = ~clk;
    sequence s(x);
        x ##1 b;
    endsequence
    property p(x);
        @(posedge clk) x |-> b;
    endproperty
    property q(sequence t);
        @(posedge clk) t;
    endproperty
    r1: assert property (p(a));
    r2: cover property (@(posedge clk) s(a));
    r3: assert property (q(s(a)));
    initial begin
        $d5;
        #12 b = 1'b0;
        #10 begin
            a = 1'b0;
            b = 1'b1;
        end
        #10 $finish;
    end
endmodule
