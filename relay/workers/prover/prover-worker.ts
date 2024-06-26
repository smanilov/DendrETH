import { Worker } from 'bullmq';
import { exec as _exec } from 'child_process';
import { ProofInputType } from '../../types/types';
import genProof from './gen_proof';
import { Redis } from '@dendreth/relay/implementations/redis';
import { Prover } from '@dendreth/relay/implementations/prover';
import { PROOF_GENERATOR_QUEUE } from '../../constants/constants';
import { checkConfig } from '@dendreth/utils/ts-utils/common-utils';
import yargs from 'yargs';

(async () => {
  const proverConfig = {
    REDIS_HOST: process.env.REDIS_HOST,
    REDIS_PORT: Number(process.env.REDIS_PORT),
  };

  checkConfig(proverConfig);

  const redis = new Redis(proverConfig.REDIS_HOST!, proverConfig.REDIS_PORT);

  const options = yargs.usage('Usage: -prover <prover>').option('prover', {
    alias: 'prover',
    describe: 'The prover server url',
    type: 'string',
    demandOption: true,
  }).argv;

  const prover = new Prover(options['prover']);

  new Worker<ProofInputType>(
    PROOF_GENERATOR_QUEUE,
    async job => genProof(redis, prover, job.data),
    {
      connection: {
        host: proverConfig.REDIS_HOST,
        port: proverConfig.REDIS_PORT,
      },
      concurrency: 1,
    },
  );
})();
