// SIM-011 A03 nearest illegal form: a const property is assigned only by
// its declaration or constructor (SV 8.19).
class A;
    const int k = 3;
    function void set();
        k = 4;
    endfunction
endclass

module tb;
    A a;

    initial begin
        a = new;
        a.set();
    end
endmodule
