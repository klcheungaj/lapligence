// IEEE 1364-2001 17.9.1-17.9.3 and IEEE 1800-2009 20.15/Annex N:
// legacy distributions update a writable seed and use the specified
// deterministic reference algorithms.
module tb;
    integer seed;
    integer implicit_a, implicit_b;
    integer random_a, random_b, random_c;
    integer uniform_a, uniform_b, uniform_c;
    integer normal_a, normal_b;
    integer exponential_a, exponential_b;
    integer poisson_a, poisson_b;
    integer chi_a, chi_b;
    integer t_a, t_b;
    integer erlang_a, erlang_b;

    initial begin
        implicit_a = $random;
        implicit_b = $random;
        $display("implicit=%0d,%0d", implicit_a, implicit_b);

        seed = 1;
        random_a = $random(seed);
        random_b = $random(seed);
        random_c = $random(seed);
        $display("random=%0d,%0d,%0d seed=%0d", random_a, random_b, random_c, seed);

        seed = 1;
        uniform_a = $dist_uniform(seed, -2, 2);
        uniform_b = $dist_uniform(seed, -2, 2);
        uniform_c = $dist_uniform(seed, -2, 2);
        $display("uniform=%0d,%0d,%0d seed=%0d", uniform_a, uniform_b, uniform_c, seed);

        seed = 10;
        normal_a = $dist_normal(seed, 10, 2);
        normal_b = $dist_normal(seed, 10, 2);
        $display("normal=%0d,%0d seed=%0d", normal_a, normal_b, seed);
        seed = 10;
        exponential_a = $dist_exponential(seed, 5);
        exponential_b = $dist_exponential(seed, 5);
        $display("exponential=%0d,%0d seed=%0d", exponential_a, exponential_b, seed);
        seed = 10;
        poisson_a = $dist_poisson(seed, 10);
        poisson_b = $dist_poisson(seed, 10);
        $display("poisson=%0d,%0d seed=%0d", poisson_a, poisson_b, seed);
        seed = 10;
        chi_a = $dist_chi_square(seed, 5);
        chi_b = $dist_chi_square(seed, 5);
        $display("chi=%0d,%0d seed=%0d", chi_a, chi_b, seed);
        seed = 10;
        t_a = $dist_t(seed, 5);
        t_b = $dist_t(seed, 5);
        $display("t=%0d,%0d seed=%0d", t_a, t_b, seed);
        seed = 10;
        erlang_a = $dist_erlang(seed, 2, 10);
        erlang_b = $dist_erlang(seed, 2, 10);
        $display("erlang=%0d,%0d seed=%0d", erlang_a, erlang_b, seed);
        $finish(0);
    end
endmodule
