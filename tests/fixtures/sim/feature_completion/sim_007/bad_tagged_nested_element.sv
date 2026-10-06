// The same nesting as the element type of a queue is rejected too.
typedef union tagged { void None; int I; string S; } value_t;
typedef struct { value_t u; int k; } holder_t;

module tb;
    holder_t q[$];

    initial begin
        q.push_back('{u: tagged S "x", k: 1});
        $display("%0d", q[0].k);
    end
endmodule
