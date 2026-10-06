// A checked member access re-reads the element's tag and member in place,
// so a side-effecting element index is rejected rather than evaluated twice.
typedef union tagged { void None; int I; string S; } value_t;

module tb;
    value_t q[$];
    int k;

    function automatic int next_index();
        k++;
        return k - 1;
    endfunction

    initial begin
        q.push_back(tagged S "x");
        $display("%s", q[next_index()].S);
    end
endmodule
