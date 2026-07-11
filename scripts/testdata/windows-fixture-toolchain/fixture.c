__declspec(dllexport) int fixture_probe(void)
{
    return 64;
}

int fixture_main(void)
{
    return fixture_probe() == 64 ? 0 : 1;
}
