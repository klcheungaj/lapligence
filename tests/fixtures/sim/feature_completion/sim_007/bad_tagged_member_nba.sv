// SIM-007 boundary: a nonblocking write to one member of a tagged union with
// a string member is legal, but SV 11.9 checks it against the tag current at
// commit (Q03), and queued native writes carry no commit-time tag check, so
// it is rejected rather than stored under another member's tag.
typedef union tagged { void None; int I; string S; } value_t;

module tb;
    value_t v;

    initial begin
        v = tagged S "a";
        v.S <= "b";
        #1 $display("%s", v.S);
        $finish(0);
    end
endmodule
